//! The Tauri command surface: one command per protocol verb the UI needs.
//!
//! These are the *only* bridge between the webview and the daemon. Schema comes
//! from the compiled-in `niripaper_proto::schema()` (decisions §4.7: the app can
//! build its panels and edit the config while the daemon is down), state/set/
//! query/kill go over the socket (PRD FR-2).

use tauri::State;

use crate::state::Connection;
use niripaper_proto::protocol::{Request, EVERY_OUTPUT};

/// Every config key a UI can offer, as JSON. Pure function, no daemon.
#[tauri::command]
pub fn daemon_schema() -> serde_json::Value {
    niripaper_proto::schema::schema()
}

/// What the daemon is doing right now, as JSON lines (one per output).
///
/// Errors come back as `Err(String)` — "the daemon is not running" is a state
/// the UI must render (PRD US-013), not a panic.
#[tauri::command]
pub fn daemon_state(
    connection: State<'_, Connection>,
    output: Option<String>,
) -> Result<String, String> {
    connection.send(&Request::State { output })
}

/// A one-line summary of what is on screen (`ok …` or `error …`).
#[tauri::command]
pub fn daemon_query(
    connection: State<'_, Connection>,
    output: Option<String>,
) -> Result<String, String> {
    connection.send(&Request::Query { output })
}

/// Switch one output's wallpaper (or every output with `*`).
#[tauri::command]
pub fn daemon_set(
    connection: State<'_, Connection>,
    output: String,
    path: String,
) -> Result<String, String> {
    let output = (output != EVERY_OUTPUT).then_some(output);
    connection.send(&Request::Set {
        output,
        path: path.into(),
    })
}

/// Switch to the next wallpaper in the playlist.
#[tauri::command]
pub fn daemon_next(
    connection: State<'_, Connection>,
    output: Option<String>,
) -> Result<String, String> {
    connection.send(&Request::Next { output })
}

/// Shut the daemon down. Returns the daemon's reply. When stopped, the Wayland
/// layer surface is destroyed and the desktop immediately reveals Noctalia.
#[tauri::command]
pub fn daemon_kill(connection: State<'_, Connection>) -> Result<String, String> {
    connection.send(&Request::Kill)
}

/// Start the niripaper daemon in the background as an independent detached process.
#[tauri::command]
pub fn daemon_start(connection: State<'_, Connection>) -> Result<String, String> {
    if connection.socket().is_ok() {
        return Ok("守护进程已在运行中".to_owned());
    }

    // Its output goes to a log, not nowhere: when it fails to start, the
    // reason is the only thing worth showing.
    let log_path = daemon_log_path();
    if let Some(dir) = log_path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let log = std::fs::File::create(&log_path)
        .map_err(|e| format!("创建日志 {} 失败: {e}", log_path.display()))?;
    let log_err = log
        .try_clone()
        .map_err(|e| format!("创建日志 {} 失败: {e}", log_path.display()))?;

    let bin = find_daemon_binary();
    let mut child = std::process::Command::new(bin)
        .arg("daemon")
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(log_err)
        .spawn()
        .map_err(|e| format!("启动守护进程失败: {e}"))?;

    // Wait briefly for socket to become available (up to 3s)
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(3000) {
        if connection.socket().is_ok() {
            // Reap it when it exits, so it does not linger as a zombie.
            std::thread::spawn(move || child.wait());
            return Ok("壁纸引擎已启动".to_owned());
        }
        if let Ok(Some(status)) = child.try_wait() {
            let text = std::fs::read_to_string(&log_path).unwrap_or_default();
            let tail: Vec<&str> = text.lines().rev().take(3).collect();
            let tail: Vec<&str> = tail.into_iter().rev().collect();
            return Err(format!(
                "守护进程启动失败（{status}）：{}",
                tail.join(" / ")
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    std::thread::spawn(move || child.wait());
    Ok(format!(
        "已发出启动指令，尚未就绪，详见 {}",
        log_path.display()
    ))
}

/// Where a daemon started from the app writes its output.
fn daemon_log_path() -> std::path::PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
        .join("niripaper")
        .join("daemon.log")
}

#[derive(serde::Serialize, Debug)]
pub struct DaemonStatusInfo {
    pub running: bool,
    pub socket_path: String,
    pub output_info: Option<String>,
}

/// Query current daemon connectivity and socket status.
#[tauri::command]
pub fn daemon_status_info(connection: State<'_, Connection>) -> DaemonStatusInfo {
    let socket = connection.socket();
    let running = socket.is_ok();
    let socket_path = match socket {
        Ok(p) => p.display().to_string(),
        Err(_) => niripaper_proto::protocol::default_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "未知".to_owned()),
    };
    let output_info = if running {
        connection.send(&Request::Query { output: None }).ok()
    } else {
        None
    };

    DaemonStatusInfo {
        running,
        socket_path,
        output_info,
    }
}

/// Point the app at a non-standard daemon socket (mirrors the daemon's own
/// `--socket`, for running a second instance in tests).
#[tauri::command]
pub fn daemon_socket(
    connection: State<'_, Connection>,
    path: Option<String>,
) -> Result<Option<String>, String> {
    let previous = connection.socket().ok().map(|p| p.display().to_string());
    connection.set_socket(path.map(std::path::PathBuf::from));
    Ok(previous)
}

/// The configuration file's path, for "open in editor" and for writing edits.
#[tauri::command]
pub fn config_path() -> Option<String> {
    niripaper_proto::config::default_path().map(|path| path.display().to_string())
}

/// Write the current palette to `~/.cache/niripaper/palette.toml`.
///
/// This is the app's *export* side: a neutral, standard Material-3 keyed file
/// that any other tool may read — a status bar, an editor theme, a compositor
/// hook. Nothing here reads anyone else's palette, and nothing writes outside
/// this one file, so niripaper keeps working on a machine with no other theming
/// daemon installed.
///
/// The UI computes the scheme (it owns the colour maths), so the tokens arrive
/// already resolved; this only serialises and writes them atomically.
#[tauri::command]
pub fn export_palette(
    seed: String,
    scheme: String,
    dark: bool,
    tokens: BTreeMap<String, String>,
) -> Result<String, String> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache")))
        .ok_or("无法确定缓存目录（XDG_CACHE_HOME / HOME 都未设置）")?;
    let dir = cache.join("niripaper");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 {} 失败: {e}", dir.display()))?;
    let path = dir.join("palette.toml");

    let mut doc = String::from(
        "# NiriPaper palette — generated from the current wallpaper.\n\
         # Standard Material 3 token names; free to read.\n\
         # NiriPaper works without this file and without anyone reading it.\n\n",
    );
    doc.push_str(&format!("seed = {seed:?}\nscheme = {scheme:?}\n"));
    doc.push_str(&format!(
        "mode = {:?}\n\n",
        if dark { "dark" } else { "light" }
    ));
    for (key, value) in &tokens {
        doc.push_str(&format!("{key} = {value:?}\n"));
    }

    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, doc).map_err(|e| {
        eprintln!("niripaper: palette write failed: {e}");
        format!("写入临时调色板失败: {e}")
    })?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        eprintln!("niripaper: palette rename failed: {e}");
        format!("替换调色板失败: {e}")
    })?;
    Ok(path.display().to_string())
}

/// Read a Material 3 palette produced by another tool, if one exists.
///
/// Read-only by design: the app never writes another program's configuration,
/// so this is safe to enable and safe to leave off. Returns `Ok(None)` when the
/// file is absent — "not installed" is a normal state, not an error.
///
/// The path is the one this environment's theme daemon renders to. It is a
/// convention, not a dependency: if the file is not there the app falls back to
/// its own generator.
#[tauri::command]
pub fn read_external_palette() -> Result<Option<BTreeMap<String, String>>, String> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache")))
        .ok_or("无法确定缓存目录")?;
    let path = cache.join("nyxuri").join("palette.toml");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let doc = text
        .parse::<DocumentMut>()
        .map_err(|e| format!("解析 {} 失败: {e}", path.display()))?;
    let mut out = BTreeMap::new();
    for (key, item) in doc.as_table().iter() {
        if let Some(value) = item.as_str() {
            out.insert(key.to_owned(), value.to_owned());
        }
    }
    if out.is_empty() {
        return Ok(None);
    }
    Ok(Some(out))
}

/// One Material 3 role set, as the system theme daemon expects it.
#[derive(serde::Deserialize)]
pub struct SystemPalette {
    pub primary: String,
    pub on_primary: String,
    pub secondary: String,
    pub on_secondary: String,
    pub tertiary: String,
    pub on_tertiary: String,
    pub error: String,
    pub on_error: String,
    pub surface: String,
    pub on_surface: String,
    pub surface_variant: String,
    pub on_surface_variant: String,
    pub outline: String,
}

/// Publish a Material 3 palette to the system theme daemon and switch to it.
///
/// Three steps, and no more: write the palette where the daemon looks, tell it
/// to use that palette, and leave everything else alone. In particular this
/// does **not** touch the wallpaper — niripaper keeps drawing it — so the two
/// programs do not fight over the Background layer.
///
/// The palette goes into the daemon's *custom* palette directory
/// (`~/.config/noctalia/palettes`), the one meant for user-supplied files. The
/// community directory is a download cache the daemon prunes, so a file put
/// there vanishes within seconds. The file is additive: it changes nothing
/// until selected, and selecting another scheme leaves it behind.
///
/// Returns the palette name that was activated.
#[tauri::command]
pub fn apply_system_palette(palette: SystemPalette) -> Result<String, String> {
    const NAME: &str = "NiriPaper";

    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .ok_or("无法确定配置目录")?;
    let dir = config.join("noctalia").join("palettes");
    let path = write_system_palette(&dir, NAME, &palette)?;

    // An error here means the daemon is not running; the file is written and
    // the next selection will find it, so report the failure rather than
    // pretending.
    let out = std::process::Command::new("noctalia")
        .args(["msg", "color-scheme-set", "custom", NAME])
        .output()
        .map_err(|e| format!("调用 noctalia 失败: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "系统主题已写入 {}，但切换失败（Noctalia 未运行？）：{}",
            path.display(),
            err.trim()
        ));
    }
    Ok(NAME.to_owned())
}

/// The file half of [`apply_system_palette`]: write `<dir>/<name>.json` in the
/// shape the daemon reads, without asking it to switch. Separate so a test can
/// check the file without changing the user's desktop.
fn write_system_palette(
    dir: &std::path::Path,
    name: &str,
    palette: &SystemPalette,
) -> Result<std::path::PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建 {} 失败: {e}", dir.display()))?;

    // Same shape a Noctalia palette file uses: one block per mode, `m`-prefixed
    // roles. Both modes get the same colours — the daemon picks by its own
    // light/dark state, and a consistent look beats guessing wrong.
    let roles = serde_json::json!({
        "mPrimary": palette.primary,
        "mOnPrimary": palette.on_primary,
        "mSecondary": palette.secondary,
        "mOnSecondary": palette.on_secondary,
        "mTertiary": palette.tertiary,
        "mOnTertiary": palette.on_tertiary,
        "mError": palette.error,
        "mOnError": palette.on_error,
        "mSurface": palette.surface,
        "mOnSurface": palette.on_surface,
        "mSurfaceVariant": palette.surface_variant,
        "mOnSurfaceVariant": palette.on_surface_variant,
        "mOutline": palette.outline,
        "mShadow": "#000000",
        "mHover": palette.surface_variant,
        "mOnHover": palette.on_surface_variant,
        // The daemon rejects a palette without a terminal block ("not found
        // or invalid" in its log), so derive one from the same roles.
        "terminal": {
            "background": palette.surface,
            "foreground": palette.on_surface,
            "cursor": palette.on_surface,
            "cursorText": palette.surface,
            "selectionBg": palette.primary,
            "selectionFg": palette.on_primary,
            "normal": {
                "black": palette.surface, "red": palette.error,
                "green": palette.secondary, "yellow": palette.tertiary,
                "blue": palette.primary, "magenta": palette.tertiary,
                "cyan": palette.secondary, "white": palette.on_surface
            },
            "bright": {
                "black": palette.outline, "red": palette.error,
                "green": palette.secondary, "yellow": palette.tertiary,
                "blue": palette.primary, "magenta": palette.tertiary,
                "cyan": palette.secondary, "white": palette.on_surface
            },
        },
    });
    let body = serde_json::json!({ "dark": roles, "light": roles }).to_string();

    let path = dir.join(format!("{name}.json"));
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, body).map_err(|e| format!("写入调色板失败: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("替换调色板失败: {e}"))?;
    Ok(path)
}

/// Check whether autostart (systemd user service or XDG autostart) is configured.
#[tauri::command]
pub fn get_autostart_status() -> bool {
    let home = match std::env::var_os("HOME") {
        Some(h) => std::path::PathBuf::from(h),
        None => return false,
    };

    let desktop_file = home.join(".config/autostart/niripaper.desktop");
    let service_file = home.join(".config/systemd/user/niripaper.service");

    desktop_file.exists() || service_file.exists()
}

/// Configure or disable autostart via systemd user service and XDG desktop entry.
#[tauri::command]
pub fn set_autostart_enabled(enabled: bool) -> Result<String, String> {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "无法获取 $HOME 目录".to_owned())?;

    let autostart_dir = home.join(".config/autostart");
    let systemd_dir = home.join(".config/systemd/user");
    let desktop_file = autostart_dir.join("niripaper.desktop");
    let service_file = systemd_dir.join("niripaper.service");

    if enabled {
        let _ = std::fs::create_dir_all(&autostart_dir);
        let _ = std::fs::create_dir_all(&systemd_dir);

        let desktop_content = "[Desktop Entry]\nType=Application\nName=NiriPaper Daemon\nComment=Parallax wallpaper daemon for Niri\nExec=niripaper daemon\nTerminal=false\nCategories=Utility;\n";
        let _ = std::fs::write(&desktop_file, desktop_content);

        let service_content = "[Unit]\nDescription=niripaper wallpaper daemon\nDocumentation=https://github.com/harukizmoe/niripaper\nAfter=graphical-session.target\nPartOf=graphical-session.target\n\n[Service]\nType=simple\nExecStart=niripaper daemon\nRestart=on-failure\nRestartSec=2\n\n[Install]\nWantedBy=graphical-session.target\n";
        let _ = std::fs::write(&service_file, service_content);

        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "enable", "niripaper.service"])
            .output();

        Ok("已开启开机自启动（已配置 systemd 用户服务与 XDG autostart）".to_owned())
    } else {
        let _ = std::fs::remove_file(&desktop_file);
        let _ = std::fs::remove_file(&service_file);

        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "niripaper.service"])
            .output();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();

        Ok("已关闭开机自启动".to_owned())
    }
}

fn find_daemon_binary() -> std::path::PathBuf {
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let candidate = parent.join("niripaper");
            if candidate.exists() {
                return candidate;
            }
        }
    }

    if let Some(home) = std::env::var_os("HOME") {
        let cargo_bin = std::path::PathBuf::from(home).join(".cargo/bin/niripaper");
        if cargo_bin.exists() {
            return cargo_bin;
        }
    }

    std::path::PathBuf::from("niripaper")
}

/// A real wallpaper file discovered on the local filesystem.
#[derive(serde::Serialize, Debug, Clone)]
pub struct LocalWallpaperItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub res: String,
    pub aspect: String,
    pub colors: Vec<String>,
    pub thumb: String,
    pub fps: Option<String>,
    pub active_on: Option<String>,
    pub is_dark: bool,
}

/// Scan `~/Pictures/Wallpapers` (or a custom directory) for real media files (§4.10).
/// Scan the library.
///
/// `async` so Tauri runs it off the webview's thread: this walks a directory,
/// decodes images and (for videos) shells out to ffmpeg, which can take
/// seconds on a cold cache. A blocking command froze the window — a blank
/// screen with no error worth reading.
#[tauri::command]
pub async fn load_local_wallpapers(
    custom_dir: Option<String>,
) -> Result<Vec<LocalWallpaperItem>, String> {
    let dir = match custom_dir {
        Some(d) => std::path::PathBuf::from(d),
        None => {
            if let Some(pictures) = std::env::var_os("XDG_PICTURES_DIR") {
                let p = std::path::PathBuf::from(pictures).join("Wallpapers");
                if p.exists() {
                    p
                } else if let Some(home) = std::env::var_os("HOME") {
                    std::path::PathBuf::from(home).join("Pictures/Wallpapers")
                } else {
                    p
                }
            } else if let Some(home) = std::env::var_os("HOME") {
                std::path::PathBuf::from(home).join("Pictures/Wallpapers")
            } else {
                std::path::PathBuf::from("Wallpapers")
            }
        }
    };

    if !dir.exists() {
        return Ok(Vec::new());
    }

    let files = walk_dir(&dir)?;

    // Decode in parallel, once per file, before the sequential pass below.
    // Colour extraction is CPU-bound and independent per file, so this turns
    // the first scan's ~20 seconds into a few; later scans hit the cache and
    // skip this entirely.
    let cache_dir = library_cache_dir();
    let _ = std::fs::create_dir_all(&cache_dir);
    {
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8);
        let queue: Vec<_> = files
            .iter()
            .filter(|p| {
                let ext = p
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                matches!(ext.as_str(), "webp" | "png" | "jpg" | "jpeg")
            })
            .cloned()
            .collect();
        let next = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..threads {
            let queue = queue.clone();
            let next = next.clone();
            let cache_dir = cache_dir.clone();
            handles.push(std::thread::spawn(move || loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= queue.len() {
                    break;
                }
                let _ = extract_colors_cached(&cache_dir, &queue[i]);
                let _ = image_thumbnail_cached(&cache_dir, &queue[i]);
            }));
        }
        for h in handles {
            let _ = h.join();
        }
    }

    let mut list = Vec::new();

    for path in files {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if name.starts_with('.') {
            continue;
        }
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();

        let (kind, is_video) = match ext.as_str() {
            "webp" | "png" | "jpg" | "jpeg" => ("static".to_owned(), false),
            "mp4" | "mkv" | "webm" | "avi" | "mov" | "gif" => ("video".to_owned(), true),
            _ => continue,
        };

        let (res, aspect, thumb_path_str) = if !is_video {
            let (w_str, asp) = if let Ok((w, h)) = image::image_dimensions(&path) {
                (format!("{w}×{h}"), format!("{w} / {h}"))
            } else {
                ("2560×1440".to_owned(), "16 / 9".to_owned())
            };
            // The cards show a small copy: decoding the original (often 4K) for
            // every card is what made the page heavy. Applying still uses `path`.
            let thumb = image_thumbnail_cached(&cache_dir, &path).unwrap_or_else(|| path.clone());
            (w_str, asp, thumb.display().to_string())
        } else {
            if let Some(t_path) = get_or_extract_video_thumbnail(&cache_dir, &path) {
                let (w_str, asp) = if let Ok((w, h)) = image::image_dimensions(&t_path) {
                    (format!("{w}×{h}"), format!("{w} / {h}"))
                } else {
                    ("1920×1080".to_owned(), "16 / 9".to_owned())
                };
                (w_str, asp, t_path.display().to_string())
            } else {
                (
                    "1920×1080".to_owned(),
                    "16 / 9".to_owned(),
                    path.display().to_string(),
                )
            }
        };

        // Prefer the picture's own colours. A video falls back to its
        // extracted thumbnail, and only a video whose frame could not be
        // extracted keeps the name-derived approximation.
        let (mut colors, is_dark) = if is_video {
            extract_colors_cached(&cache_dir, std::path::Path::new(&thumb_path_str))
        } else {
            extract_colors_cached(&cache_dir, &path)
        };
        if colors.is_empty() {
            colors = generate_colors(&name);
        }
        let id = format!("local-{}", hash_str(&name));
        let path_str = path.display().to_string();

        list.push(LocalWallpaperItem {
            id,
            name,
            path: path_str,
            kind,
            res,
            aspect,
            colors,
            thumb: thumb_path_str,
            fps: if is_video {
                Some("30 FPS".to_owned())
            } else {
                None
            },
            active_on: None,
            is_dark,
        });
    }

    list.sort_by_key(|a| a.name.to_lowercase());
    Ok(list)
}

fn walk_dir(root: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    let mut dirs_to_visit = vec![root.to_path_buf()];
    while let Some(current_dir) = dirs_to_visit.pop() {
        if let Ok(entries) = std::fs::read_dir(&current_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if name.starts_with('.') || name == "thumbnails" {
                    continue;
                }
                if p.is_dir() {
                    dirs_to_visit.push(p);
                } else if p.is_file() {
                    files.push(p);
                }
            }
        }
    }
    Ok(files)
}
fn get_or_extract_video_thumbnail(
    cache_dir: &std::path::Path,
    video_path: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let _ = std::fs::create_dir_all(cache_dir);
    let h = format!("{:016x}", hash_str(&video_path.to_string_lossy()));
    let thumb_path = cache_dir.join(format!("{h}.jpg"));

    if thumb_path.exists() {
        return Some(thumb_path);
    }

    // Extract 1 frame at 00:00:01 using ffmpeg, scaling to 720 width
    let status = std::process::Command::new("ffmpeg")
        .arg("-y")
        .arg("-ss")
        .arg("00:00:01")
        .arg("-i")
        .arg(video_path)
        .arg("-vframes")
        .arg("1")
        .arg("-vf")
        .arg("scale=720:-1")
        .arg(&thumb_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    if let Ok(st) = status {
        if st.success() && thumb_path.exists() {
            return Some(thumb_path);
        }
    }
    None
}

/// Where previews and extracted colours live: the user's cache directory, not
/// the wallpaper folder, which other programs (the desktop shell) scan too and
/// would list every preview as a wallpaper of its own.
fn library_cache_dir() -> std::path::PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
        .join("niripaper")
        .join("library")
}

/// Width of the card previews, the same as the video thumbnails.
const THUMB_WIDTH: u32 = 720;

/// A small JPEG copy of an image for the library cards, made once and reused.
/// The name carries the file's size and modification time, so an edited file
/// gets a fresh preview instead of a stale one.
fn image_thumbnail_cached(
    cache_dir: &std::path::Path,
    image_path: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let meta = std::fs::metadata(image_path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let key = format!(
        "{}|{}|{}",
        image_path.to_string_lossy(),
        meta.len(),
        modified
    );
    let thumb_path = cache_dir.join(format!("img-{:016x}.jpg", hash_str(&key)));
    if thumb_path.exists() {
        return Some(thumb_path);
    }

    let img = image::open(image_path).ok()?;
    // Never enlarge a small image.
    let width = img.width().min(THUMB_WIDTH);
    let height =
        (u64::from(img.height()) * u64::from(width) / u64::from(img.width().max(1))).max(1) as u32;
    let small = img.thumbnail(width, height).to_rgb8();
    // Write beside the target and rename, so a reader never sees half a file.
    let tmp = thumb_path.with_extension("tmp");
    small
        .save_with_format(&tmp, image::ImageFormat::Jpeg)
        .ok()?;
    std::fs::rename(&tmp, &thumb_path).ok()?;
    Some(thumb_path)
}

fn hash_str(s: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        h = (h ^ (b as u64)).wrapping_mul(0x100000001b3);
    }
    h
}

/// Cached wrapper around [`extract_colors`].
///
/// Decoding a directory of 4K wallpapers is expensive, and the scan runs every
/// time the app starts or the user refreshes. The palette depends only on the
/// file's bytes, so key it by path + size + mtime and keep the result next to
/// the video thumbnails. A cache miss costs one decode; every later scan is a
/// handful of small reads.
fn extract_colors_cached(
    cache_dir: &std::path::Path,
    path: &std::path::Path,
) -> (Vec<String>, bool) {
    let meta = std::fs::metadata(path).ok();
    let key = format!(
        "{:016x}",
        hash_str(&format!(
            "{}|{}|{}",
            path.display(),
            meta.as_ref().map(|m| m.len()).unwrap_or(0),
            meta.as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0)
        ))
    );
    // `v3`: includes calculated `is_dark` tag on line 1, then ranked colors.
    let cache_file = cache_dir.join(format!("colors-v3-{key}.txt"));

    if let Ok(text) = std::fs::read_to_string(&cache_file) {
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        if let Some(first) = lines.next() {
            let is_dark = first == "dark";
            let colors: Vec<String> = lines.map(str::to_owned).collect();
            if !colors.is_empty() {
                return (colors, is_dark);
            }
        }
    }

    let (colors, is_dark) = extract_colors_and_luma(path);
    if !colors.is_empty() {
        let content = format!(
            "{}\n{}",
            if is_dark { "dark" } else { "light" },
            colors.join("\n")
        );
        let _ = std::fs::write(&cache_file, content);
    }
    (colors, is_dark)
}

/// Rank the image's candidate theme source colours, best first, and compute
/// perceived luminance for automatic dark/light classification.
fn extract_colors_and_luma(path: &std::path::Path) -> (Vec<String>, bool) {
    use material_colors::color::Rgb;
    use material_colors::quantize::{Quantizer, QuantizerCelebi};
    use material_colors::score::Score;

    let Ok(img) = image::open(path) else {
        return (Vec::new(), false);
    };
    // Downscale first, as the library asks: 128x128 keeps every region of the
    // picture represented and quantises in a few milliseconds.
    let small = img.resize_exact(128, 128, image::imageops::FilterType::Triangle);
    let rgb = small.to_rgb8();

    // Calculate full-image perceived relative luminance (Rec. 709).
    let mut sum_luma = 0.0f64;
    for p in rgb.pixels() {
        sum_luma += 0.2126 * p.0[0] as f64 + 0.7152 * p.0[1] as f64 + 0.0722 * p.0[2] as f64;
    }
    let avg_luma = sum_luma / (128.0 * 128.0);
    let is_dark = avg_luma < 108.0;

    let pixels: Vec<Rgb> = rgb
        .pixels()
        .map(|p| Rgb::new(p.0[0], p.0[1], p.0[2]))
        .collect();
    let clusters = QuantizerCelebi::quantize(&pixels, 128);
    // `filter = true` drops greys and slivers; when nothing survives, Score
    // returns its fallback, which is an honest "no usable colour" seed.
    let colors = Score::score(&clusters.color_to_count, Some(4), None, Some(true))
        .iter()
        .map(|c| format!("#{}", c.as_hex()))
        .collect();
    (colors, is_dark)
}

#[cfg(test)]
fn extract_colors(path: &std::path::Path) -> Vec<String> {
    extract_colors_and_luma(path).0
}

/// Older call site kept for videos, where there is no still frame to sample.
fn generate_colors(name: &str) -> Vec<String> {
    let h = hash_str(name);
    let hues = [
        (h % 360) as f64,
        ((h + 50) % 360) as f64,
        ((h + 110) % 360) as f64,
        ((h + 190) % 360) as f64,
        ((h + 260) % 360) as f64,
    ];
    hues.iter()
        .map(|&hue| hsl_to_hex(hue, 0.65, 0.45))
        .collect()
}

fn hsl_to_hex(h: f64, s: f64, l: f64) -> String {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    format!(
        "#{:02x}{:02x}{:02x}",
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8
    )
}

/// Wallhaven API search query parameters (matching Noctalia's official plugin).
#[derive(serde::Deserialize, Debug)]
pub struct WallhavenQueryParams {
    pub q: Option<String>,
    pub categories: Option<String>,
    pub purity: Option<String>,
    pub sorting: Option<String>,
    pub order: Option<String>,
    pub page: Option<u32>,
    pub ratios: Option<String>,
    pub atleast: Option<String>,
    pub apikey: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WallhavenItem {
    pub id: String,
    pub name: String,
    pub res: String,
    pub ratio: String,
    pub category: String,
    pub purity: String,
    pub favs: String,
    pub aspect: String,
    pub colors: Vec<String>,
    pub thumb: String,
    pub full_url: String,
    pub file_type: String,
    pub tags: Vec<String>,
}

#[derive(serde::Serialize, Debug)]
pub struct WallhavenSearchResult {
    pub wallpapers: Vec<WallhavenItem>,
    pub current_page: u32,
    pub last_page: u32,
    pub total: u64,
}

/// Query Wallhaven API v1 directly via curl (zero extra dependencies, uses system TLS).
///
/// `async` on purpose: a plain `fn` command runs on the main thread, and this one
/// waits on curl for a second or so — the whole window, spinner included, froze
/// for that long on every page. The blocking work goes to a worker thread.
#[tauri::command]
pub async fn search_wallhaven(
    params: WallhavenQueryParams,
) -> Result<WallhavenSearchResult, String> {
    tauri::async_runtime::spawn_blocking(move || search_wallhaven_blocking(params))
        .await
        .map_err(|e| format!("检索线程异常: {e}"))?
}

fn search_wallhaven_blocking(
    params: WallhavenQueryParams,
) -> Result<WallhavenSearchResult, String> {
    let mut query_parts = Vec::new();

    if let Some(q) = params.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        query_parts.push(format!("q={}", url_encode(q)));
    }
    if let Some(cat) = params.categories.as_deref() {
        query_parts.push(format!("categories={cat}"));
    }
    if let Some(pur) = params.purity.as_deref() {
        query_parts.push(format!("purity={pur}"));
    }
    if let Some(sort) = params.sorting.as_deref() {
        query_parts.push(format!("sorting={sort}"));
    }
    if let Some(order) = params.order.as_deref() {
        query_parts.push(format!("order={order}"));
    }
    let page = params.page.unwrap_or(1);
    query_parts.push(format!("page={page}"));

    if let Some(ratios) = params.ratios.as_deref().filter(|s| !s.is_empty()) {
        query_parts.push(format!("ratios={ratios}"));
    }
    if let Some(atleast) = params.atleast.as_deref().filter(|s| !s.is_empty()) {
        query_parts.push(format!("atleast={atleast}"));
    }
    if let Some(key) = params.apikey.as_deref().filter(|s| !s.is_empty()) {
        query_parts.push(format!("apikey={key}"));
    }

    let url = format!(
        "https://wallhaven.cc/api/v1/search?{}",
        query_parts.join("&")
    );

    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-L",
            "--max-time",
            "12",
            "-A",
            "niripaper/0.1.0",
            &url,
        ])
        .output()
        .map_err(|e| format!("执行 curl 失败: {e}"))?;

    if !output.status.success() {
        return Err(format!("网络请求失败，退出码: {:?}", output.status.code()));
    }

    let json_text = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&json_text).map_err(|e| format!("解析 Wallhaven 响应失败: {e}"))?;

    if let Some(err_msg) = parsed.get("error").and_then(|v| v.as_str()) {
        return Err(format!("Wallhaven API: {err_msg}"));
    }

    let data = parsed
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "响应中缺少 data 数组".to_owned())?;

    let meta = parsed.get("meta");
    let current_page = meta
        .and_then(|m| m.get("current_page"))
        .and_then(|v| v.as_u64())
        .unwrap_or(page as u64) as u32;
    let last_page = meta
        .and_then(|m| m.get("last_page"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u32;
    let total = meta
        .and_then(|m| m.get("total"))
        .and_then(|v| v.as_u64())
        .unwrap_or(data.len() as u64);

    let mut wallpapers = Vec::with_capacity(data.len());
    for item in data {
        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned();
        let res = item
            .get("resolution")
            .and_then(|v| v.as_str())
            .unwrap_or("2560×1440")
            .replace('x', "×");
        let ratio = item
            .get("ratio")
            .and_then(|v| v.as_str())
            .unwrap_or("16:9")
            .to_owned();
        let dim_x = item
            .get("dimension_x")
            .and_then(|v| v.as_u64())
            .unwrap_or(16);
        let dim_y = item
            .get("dimension_y")
            .and_then(|v| v.as_u64())
            .unwrap_or(9);
        let aspect = format!("{dim_x} / {dim_y}");
        let category = item
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("General")
            .to_owned();
        let purity = item
            .get("purity")
            .and_then(|v| v.as_str())
            .unwrap_or("SFW")
            .to_uppercase();
        let favorites = item.get("favorites").and_then(|v| v.as_u64()).unwrap_or(0);
        let favs = if favorites >= 1000 {
            format!("{:.1}k", favorites as f64 / 1000.0)
        } else {
            favorites.to_string()
        };
        let colors = item
            .get("colors")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let full_url = item
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned();
        let thumb = item
            .get("thumbs")
            .and_then(|t| t.get("large").or_else(|| t.get("original")))
            .and_then(|v| v.as_str())
            .unwrap_or(&full_url)
            .to_owned();
        let file_type = item
            .get("file_type")
            .and_then(|v| v.as_str())
            .unwrap_or("image/jpeg")
            .to_owned();
        let name = format!("wallhaven-{}", id);

        wallpapers.push(WallhavenItem {
            id,
            name,
            res,
            ratio,
            category,
            purity,
            favs,
            aspect,
            colors,
            thumb,
            full_url,
            file_type,
            tags: Vec::new(),
        });
    }

    Ok(WallhavenSearchResult {
        wallpapers,
        current_page,
        last_page,
        total,
    })
}

/// Download a Wallhaven wallpaper into `~/Pictures/Wallpapers` (matching Noctalia's download workflow).
/// Off the main thread for the same reason as [`search_wallhaven`].
#[tauri::command]
pub async fn download_wallhaven_wallpaper(
    full_url: String,
    filename: String,
    dest_dir: Option<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        download_wallhaven_blocking(full_url, filename, dest_dir)
    })
    .await
    .map_err(|e| format!("下载线程异常: {e}"))?
}

fn download_wallhaven_blocking(
    full_url: String,
    filename: String,
    dest_dir: Option<String>,
) -> Result<String, String> {
    let target_dir = match dest_dir {
        Some(d) => std::path::PathBuf::from(d),
        None => {
            if let Some(pictures) = std::env::var_os("XDG_PICTURES_DIR") {
                let p = std::path::PathBuf::from(pictures).join("Wallpapers");
                if p.exists() {
                    p
                } else if let Some(home) = std::env::var_os("HOME") {
                    std::path::PathBuf::from(home).join("Pictures/Wallpapers")
                } else {
                    p
                }
            } else if let Some(home) = std::env::var_os("HOME") {
                std::path::PathBuf::from(home).join("Pictures/Wallpapers")
            } else {
                std::path::PathBuf::from("Wallpapers")
            }
        }
    };

    std::fs::create_dir_all(&target_dir).map_err(|e| format!("创建目录失败: {e}"))?;
    let dest_path = target_dir.join(&filename);

    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-L",
            "--max-time",
            "60",
            "-A",
            "niripaper/0.1.0",
            "-o",
            &dest_path.display().to_string(),
            &full_url,
        ])
        .output()
        .map_err(|e| format!("下载壁纸失败: {e}"))?;

    if !output.status.success() {
        return Err("下载壁纸请求失败".to_owned());
    }

    Ok(dest_path.display().to_string())
}

fn url_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

use std::collections::BTreeMap;
use toml_edit::{DocumentMut, Item};

/// Set `key` in `table` **keeping the comments attached to it**.
///
/// `TableLike::insert` replaces the whole item, and the item owns its leading
/// comments — so a plain `insert` silently deletes the documentation above a
/// key. Verified: inserting over `# comment ON b\nb = 2` leaves `b = 20` with
/// the comment gone. Assigning through the index form (`table[key] = value`)
/// edits the value in place and keeps the decoration, so every write here goes
/// through this helper. The shipped `config.toml` is heavily commented and the
/// project promises those comments survive a write.
fn set_keep_comments(table: &mut dyn toml_edit::TableLike, key: &str, value: toml_edit::Item) {
    if let Some(existing) = table.get_mut(key) {
        *existing = value;
    } else {
        table.insert(key, value);
    }
}

pub(crate) fn read_config_doc() -> Result<(std::path::PathBuf, DocumentMut), String> {
    let path = niripaper_proto::config::default_path()
        .ok_or_else(|| "无法获取 config.toml 路径".to_owned())?;
    let text = if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| format!("读取 config.toml 失败: {e}"))?
    } else {
        String::new()
    };
    let doc = text
        .parse::<DocumentMut>()
        .map_err(|e| format!("解析 config.toml 失败: {e}"))?;
    Ok((path, doc))
}

pub(crate) fn write_config_atomic(path: &std::path::Path, doc: &DocumentMut) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, doc.to_string()).map_err(|e| format!("写入临时配置失败: {e}"))?;
    std::fs::rename(&temp_path, path).map_err(|e| format!("替换配置文件失败: {e}"))?;
    Ok(())
}

/// The write half of [`daemon_apply`], as a pure function of the document, so
/// it can be tested without a daemon or a real config file.
///
/// `*` writes the global keys; a name writes that output's table. Both are the
/// same two keys, which is why one helper serves both.
fn apply_wallpaper_to_doc(
    doc: &mut DocumentMut,
    output: &str,
    path: &str,
    fit: Option<&str>,
    scale: Option<f64>,
) -> Result<(), String> {
    let write = |table: &mut dyn toml_edit::TableLike| {
        set_keep_comments(table, "wallpaper", toml_edit::value(path));
        if let Some(fit) = fit {
            set_keep_comments(table, "fit", toml_edit::value(fit));
        }
    };

    if output == EVERY_OUTPUT {
        write(doc.as_table_mut());
        // "Every output" has to mean it: an output's own `wallpaper` overrides
        // the global one, so leaving it would bring the old picture back on
        // that monitor at the next reload. Its other keys stay.
        if let Some(outputs) = doc.get_mut("outputs").and_then(|i| i.as_table_like_mut()) {
            for (_, table) in outputs.iter_mut() {
                if let Some(table) = table.as_table_like_mut() {
                    table.remove("wallpaper");
                    if fit.is_some() {
                        table.remove("fit");
                    }
                }
            }
        }
    } else {
        if !doc.contains_key("outputs") {
            doc["outputs"] = Item::Table(toml_edit::Table::new());
        }
        let outputs = doc
            .get_mut("outputs")
            .and_then(|i| i.as_table_like_mut())
            .ok_or("config.toml 的 outputs 不是表")?;
        if !outputs.contains_key(output) {
            outputs.insert(output, Item::Table(toml_edit::Table::new()));
        }
        let table = outputs
            .get_mut(output)
            .and_then(|i| i.as_table_like_mut())
            .ok_or_else(|| format!("outputs.{output} 不是表"))?;
        write(table);
    }

    // `scale` is global only (the parallax operates on one shared canvas).
    if let Some(scale) = scale {
        doc["scale"] = toml_edit::value(scale.clamp(1.0, 1.35));
    }
    Ok(())
}

/// Apply a wallpaper *and* write it to `config.toml`, so a restart comes back
/// to the same wallpaper and fit. `set` alone only swaps what is on screen (§2:
/// it deliberately does not touch the configuration) — that is why the choice
/// used to be forgotten on restart.
#[tauri::command]
pub fn daemon_apply(
    connection: State<'_, Connection>,
    output: String,
    path: String,
    fit: Option<String>,
    scale: Option<f64>,
) -> Result<String, String> {
    // Write first: if the configuration cannot be saved, say so before the
    // screen changes, so what is on screen and what is on disk never diverge.
    let (config_path, mut doc) = read_config_doc()?;
    apply_wallpaper_to_doc(&mut doc, &output, &path, fit.as_deref(), scale)?;
    write_config_atomic(&config_path, &doc)?;

    // Then swap what is on screen, through the same path `set` uses.
    let target = (output != EVERY_OUTPUT).then_some(output);
    connection.send(&Request::Set {
        output: target,
        path: path.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The drawer used to offer `cover`, which is not one of the daemon's fits
    /// (`fill` is). Writing it straight through made the daemon reject the
    /// whole configuration on reload — so the wallpaper looked applied and then
    /// vanished on the next restart. This pins the names the panel may send.
    #[test]
    fn every_drawer_fit_name_is_a_real_fit() {
        for name in ["fill", "fit", "stretch", "center", "tile"] {
            assert!(
                niripaper_proto::domain::Fit::parse(name).is_ok(),
                "{name} must be a daemon fit"
            );
        }
        assert!(niripaper_proto::domain::Fit::parse("cover").is_err());
    }

    /// Applying a wallpaper has to survive a restart, which means it has to be
    /// in the document — not only on screen.
    #[test]
    fn applying_writes_the_global_keys() {
        let mut doc = DocumentMut::new();
        apply_wallpaper_to_doc(&mut doc, EVERY_OUTPUT, "/w/a.png", Some("fit"), None).unwrap();
        assert_eq!(doc["wallpaper"].as_str(), Some("/w/a.png"));
        assert_eq!(doc["fit"].as_str(), Some("fit"));
    }

    /// A named output goes into its own table, and the global keys are left
    /// alone: applying to one monitor must not repaint the others on restart.
    #[test]
    fn applying_to_one_output_does_not_touch_the_globals() {
        let mut doc = "wallpaper = \"/w/global.png\"\n"
            .parse::<DocumentMut>()
            .unwrap();
        apply_wallpaper_to_doc(&mut doc, "DP-1", "/w/dp1.png", Some("fill"), None).unwrap();
        assert_eq!(doc["wallpaper"].as_str(), Some("/w/global.png"));
        assert_eq!(
            doc["outputs"]["DP-1"]["wallpaper"].as_str(),
            Some("/w/dp1.png")
        );
        assert_eq!(doc["outputs"]["DP-1"]["fit"].as_str(), Some("fill"));
    }

    /// Applying to every output must not leave an output's own wallpaper behind
    /// to override it; that output's other settings are kept.
    #[test]
    fn applying_to_every_output_clears_the_per_output_wallpapers() {
        let mut doc = "[outputs.DP-1]\nwallpaper = \"/w/old.png\"\nfit = \"fill\"\nscale = 1.1\n"
            .parse::<DocumentMut>()
            .unwrap();
        apply_wallpaper_to_doc(&mut doc, EVERY_OUTPUT, "/w/new.png", Some("fit"), None).unwrap();
        assert_eq!(doc["wallpaper"].as_str(), Some("/w/new.png"));
        assert!(doc["outputs"]["DP-1"].get("wallpaper").is_none());
        assert!(doc["outputs"]["DP-1"].get("fit").is_none());
        assert_eq!(doc["outputs"]["DP-1"]["scale"].as_float(), Some(1.1));
    }

    /// Comments are the reason this writes through `toml_edit` at all; a
    /// round-trip that dropped them would silently destroy the documented
    /// config the project ships.
    #[test]
    fn applying_keeps_the_comments() {
        let mut doc = "# keep me\nwallpaper = \"/w/a.png\"\n"
            .parse::<DocumentMut>()
            .unwrap();
        apply_wallpaper_to_doc(&mut doc, EVERY_OUTPUT, "/w/b.png", None, None).unwrap();
        assert!(doc.to_string().contains("# keep me"));
        assert!(doc.to_string().contains("\"/w/b.png\""));
    }

    /// The scale clamp is the daemon's `MAX_SCALE`: writing a larger one would
    /// make the file fail to parse on the very next reload.
    #[test]
    fn applying_clamps_scale_to_the_supported_range() {
        let mut doc = DocumentMut::new();
        apply_wallpaper_to_doc(&mut doc, EVERY_OUTPUT, "/w/a.png", None, Some(9.0)).unwrap();
        assert_eq!(doc["scale"].as_float(), Some(1.35));

        let mut doc = DocumentMut::new();
        apply_wallpaper_to_doc(&mut doc, EVERY_OUTPUT, "/w/a.png", None, Some(0.2)).unwrap();
        assert_eq!(doc["scale"].as_float(), Some(1.0));
    }
}

#[cfg(test)]
mod color_tests {
    use super::*;

    /// The palette must come from the pixels, not from the file name. Two
    /// different images must not produce the same hue, and the same image must
    /// produce the same palette twice.
    #[test]
    fn extracted_colours_track_the_image() {
        let dir = std::env::temp_dir().join(format!("niripaper-color-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        // A saturated red field and a saturated blue field.
        let mut red = image::RgbImage::new(32, 32);
        for px in red.pixels_mut() {
            *px = image::Rgb([200, 30, 30]);
        }
        let red_path = dir.join("red.png");
        red.save(&red_path).unwrap();

        let mut blue = image::RgbImage::new(32, 32);
        for px in blue.pixels_mut() {
            *px = image::Rgb([30, 30, 200]);
        }
        let blue_path = dir.join("blue.png");
        blue.save(&blue_path).unwrap();

        let red_colors = extract_colors(&red_path);
        let blue_colors = extract_colors(&blue_path);
        assert!(!red_colors.is_empty() && !blue_colors.is_empty());
        assert_ne!(
            red_colors[0], blue_colors[0],
            "different images, different colours"
        );

        // Dominant channel is preserved.
        let r = &red_colors[0];
        let red_channel = u8::from_str_radix(&r[1..3], 16).unwrap();
        let blue_channel = u8::from_str_radix(&r[5..7], 16).unwrap();
        assert!(red_channel > blue_channel, "red image gave {r}");

        // Deterministic.
        assert_eq!(red_colors, extract_colors(&red_path));

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod palette_push_tests {
    use super::*;

    /// The palette must be written in the exact shape the daemon expects. Only
    /// the file half runs: the full command would switch the user's desktop.
    #[test]
    fn writes_a_custom_palette() {
        let dir = std::env::temp_dir().join(format!("niripaper-pal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let palette = SystemPalette {
            primary: "#115e7f".into(),
            on_primary: "#ffffff".into(),
            secondary: "#8ed6ac".into(),
            on_secondary: "#000000".into(),
            tertiary: "#cacc58".into(),
            on_tertiary: "#000000".into(),
            error: "#ffb4ab".into(),
            on_error: "#000000".into(),
            surface: "#121318".into(),
            on_surface: "#e3e1e8".into(),
            surface_variant: "#1f1f24".into(),
            on_surface_variant: "#c5c6d2".into(),
            outline: "#8f909b".into(),
        };
        let path = write_system_palette(&dir, "NiriPaper", &palette).expect("palette written");
        assert_eq!(path, dir.join("NiriPaper.json"));
        let text = std::fs::read_to_string(&path).expect("palette written");
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(parsed["dark"]["mPrimary"], "#115e7f");
        assert_eq!(parsed["light"]["mPrimary"], "#115e7f");
        assert_eq!(parsed["dark"]["mShadow"], "#000000");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
