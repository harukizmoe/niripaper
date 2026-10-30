//! Wallhaven thumbnail cache.
//!
//! The masonry keeps only the cards near the viewport mounted, so scrolling
//! back re-mounts them. Pointing those `<img>` elements at the remote URL again
//! meant a fresh request for a picture the user had already seen — the last
//! source of stutter while browsing. Thumbnails are small, so each one is
//! downloaded once into `~/.cache/niripaper/wallhaven/` and served from disk
//! afterwards.
//!
//! Downloads go through `curl` for the same reason the API calls do: no extra
//! dependency, and the system's own TLS stack.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

/// Downloads in flight at once. One screen holds around a dozen thumbnails;
/// four at a time fills it quickly without opening dozens of connections to
/// the thumb CDN.
const PARALLEL_DOWNLOADS: usize = 4;

/// The local file for a Wallhaven thumbnail, downloading it on first use.
///
/// `async` so the wait does not sit on the main thread (see `commands.rs` for
/// why that matters in Tauri).
#[tauri::command]
pub async fn wallhaven_thumbnail(id: String, url: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || cached_or_download(&id, &url))
        .await
        .map_err(|e| format!("缩略图线程异常: {e}"))?
        .map(|path| path.to_string_lossy().into_owned())
}

fn cache_dir() -> Result<PathBuf, String> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .ok_or_else(|| "无法确定缓存目录（没有 XDG_CACHE_HOME / HOME）".to_owned())?;
    Ok(base.join("niripaper").join("wallhaven"))
}

/// `<id>.<ext>`. The id comes from the API and ends up in a file name, so
/// anything outside `[A-Za-z0-9_-]` is refused rather than escaped.
fn file_name(id: &str, url: &str) -> Option<String> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    let last = url.split('?').next().unwrap_or(url).rsplit('/').next()?;
    let ext = match last.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()) {
        Some(ext)
            if matches!(
                ext.as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "avif" | "gif"
            ) =>
        {
            ext
        }
        _ => "jpg".to_owned(),
    };
    Some(format!("{id}.{ext}"))
}

fn cached_or_download(id: &str, url: &str) -> Result<PathBuf, String> {
    let dir = cache_dir()?;
    let name = file_name(id, url).ok_or_else(|| format!("无效的缩略图标识: {id}"))?;
    let path = dir.join(&name);
    if path.is_file() {
        return Ok(path);
    }

    std::fs::create_dir_all(&dir).map_err(|e| format!("创建缩略图缓存目录失败: {e}"))?;
    let _slot = Slot::acquire();

    // While this request waited for a free slot another card may have brought
    // the same picture in; the file is the signal, no bookkeeping needed.
    if path.is_file() {
        return Ok(path);
    }

    // Written under a temporary name and renamed into place, so a killed
    // download never leaves a half file that looks cached.
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tmp = dir.join(format!(
        "{name}.{}.{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let status = std::process::Command::new("curl")
        .args([
            "-s",
            "-L",
            "--max-time",
            "20",
            "-A",
            "niripaper/0.1.0",
            "-o",
        ])
        .arg(&tmp)
        .arg(url)
        .status()
        .map_err(|e| format!("执行 curl 失败: {e}"))?;

    let complete = status.success() && tmp.metadata().map(|m| m.len() > 0).unwrap_or(false);
    if !complete {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("下载缩略图失败: {url}"));
    }
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("写入缩略图缓存失败: {e}")
    })?;
    Ok(path)
}

/// A download permit. Dropping it hands the slot to the next waiter.
struct Slot;

impl Slot {
    fn acquire() -> Self {
        let (lock, cv) = gate();
        let mut running = lock.lock().unwrap_or_else(|e| e.into_inner());
        while *running >= PARALLEL_DOWNLOADS {
            running = cv.wait(running).unwrap_or_else(|e| e.into_inner());
        }
        *running += 1;
        Self
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let (lock, cv) = gate();
        let mut running = lock.lock().unwrap_or_else(|e| e.into_inner());
        *running = running.saturating_sub(1);
        cv.notify_one();
    }
}

fn gate() -> &'static (Mutex<usize>, Condvar) {
    static GATE: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();
    GATE.get_or_init(|| (Mutex::new(0), Condvar::new()))
}

#[cfg(test)]
mod tests {
    use super::file_name;

    #[test]
    fn names_files_after_the_id_with_the_urls_extension() {
        let url = "https://th.wallhaven.cc/lg/xx/xxxxx.jpg?foo=bar";
        assert_eq!(file_name("abc123", url).as_deref(), Some("abc123.jpg"));
        assert_eq!(
            file_name("abc123", "https://th.wallhaven.cc/lg/xx/xxxxx.PNG").as_deref(),
            Some("abc123.png")
        );
    }

    #[test]
    fn unknown_or_absent_extensions_fall_back_to_jpg() {
        assert_eq!(
            file_name("abc", "https://x/y/thumb").as_deref(),
            Some("abc.jpg")
        );
        assert_eq!(
            file_name("abc", "https://x/y/thumb.svg").as_deref(),
            Some("abc.jpg")
        );
    }

    #[test]
    fn ids_that_could_escape_the_cache_directory_are_refused() {
        assert_eq!(
            file_name("../../etc/passwd", "https://x/y.jpg").map(|s| s.len()),
            None
        );
        assert_eq!(file_name("a/b", "https://x/y.jpg").map(|s| s.len()), None);
        assert_eq!(file_name("", "https://x/y.jpg").map(|s| s.len()), None);
    }
}
