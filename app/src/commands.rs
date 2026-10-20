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

    let bin = find_daemon_binary();
    let _child = std::process::Command::new(bin)
        .arg("daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("启动守护进程失败: {e}"))?;

    // Wait briefly for socket to become available (up to 1.5s)
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(1500) {
        if connection.socket().is_ok() {
            return Ok("壁纸引擎已成功启动并在后台独立运行".to_owned());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok("守护进程指令已发出".to_owned())
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
}

/// Scan `~/Pictures/Wallpapers` (or a custom directory) for real media files (§4.10).
#[tauri::command]
pub fn load_local_wallpapers(
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

        let (res, aspect) = if !is_video {
            if let Ok((w, h)) = image::image_dimensions(&path) {
                (format!("{w}×{h}"), format!("{w} / {h}"))
            } else {
                ("2560×1440".to_owned(), "16 / 9".to_owned())
            }
        } else {
            ("1920×1080".to_owned(), "16 / 9".to_owned())
        };

        let colors = generate_colors(&name);
        let id = format!("local-{}", hash_str(&name));
        let path_str = path.display().to_string();

        list.push(LocalWallpaperItem {
            id,
            name,
            path: path_str.clone(),
            kind,
            res,
            aspect,
            colors,
            thumb: path_str,
            fps: if is_video {
                Some("30 FPS".to_owned())
            } else {
                None
            },
            active_on: None,
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

fn hash_str(s: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        h = (h ^ (b as u64)).wrapping_mul(0x100000001b3);
    }
    h
}

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
#[tauri::command]
pub fn search_wallhaven(params: WallhavenQueryParams) -> Result<WallhavenSearchResult, String> {
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
#[tauri::command]
pub fn download_wallhaven_wallpaper(
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

use toml_edit::{DocumentMut, Item};

/// Parallax and layout-reactive motion configuration parameters (§4).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ParallaxConfig {
    pub enabled: bool,
    pub scale: f64,
    pub column_span: usize,
    pub workspace_span: usize,
    pub duration_ms: u64,
    pub curve: String,
    pub follow_niri: bool,
    pub overview_zoom: f64,
}

fn read_config_doc() -> Result<(std::path::PathBuf, DocumentMut), String> {
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

fn write_config_atomic(path: &std::path::Path, doc: &DocumentMut) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, doc.to_string()).map_err(|e| format!("写入临时配置失败: {e}"))?;
    std::fs::rename(&temp_path, path).map_err(|e| format!("替换配置文件失败: {e}"))?;
    Ok(())
}

/// Read the real parallax configuration directly from `config.toml`.
#[tauri::command]
pub fn get_parallax_config() -> Result<ParallaxConfig, String> {
    let (_, doc) = read_config_doc()?;

    let scale_val = doc.get("scale").and_then(|v| v.as_float()).unwrap_or(1.1);
    let column_span = doc
        .get("column_span")
        .and_then(|v| v.as_integer())
        .unwrap_or(6) as usize;
    let workspace_span = doc
        .get("workspace_span")
        .and_then(|v| v.as_integer())
        .unwrap_or(6) as usize;

    let anim_table = doc.get("animations").and_then(|i| i.as_table_like());
    let follow_niri = anim_table
        .and_then(|t| t.get("follow_niri"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let global_off = anim_table
        .and_then(|t| t.get("off"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let parallax_table = anim_table
        .and_then(|t| t.get("parallax"))
        .and_then(|i| i.as_table_like());
    let parallax_off = parallax_table
        .and_then(|t| t.get("off"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let duration_ms = parallax_table
        .and_then(|t| t.get("duration_ms"))
        .and_then(|v| v.as_integer())
        .unwrap_or(600) as u64;
    let curve = parallax_table
        .and_then(|t| t.get("curve"))
        .and_then(|v| v.as_str())
        .unwrap_or("ease-out-cubic")
        .to_owned();

    let overview_table = anim_table
        .and_then(|t| t.get("overview-open-close"))
        .and_then(|i| i.as_table_like());
    let overview_zoom = overview_table
        .and_then(|t| t.get("zoom"))
        .and_then(|v| v.as_float())
        .unwrap_or(0.96);

    let enabled = scale_val > 1.001 && !global_off && !parallax_off;

    Ok(ParallaxConfig {
        enabled,
        scale: scale_val,
        column_span,
        workspace_span,
        duration_ms,
        curve,
        follow_niri,
        overview_zoom,
    })
}

/// Toggle the real parallax effect on or off in `config.toml` (saving atomically).
/// When off: sets scale = 1.0 and [animations.parallax] off = true.
/// When on: sets scale = 1.10 and enables parallax easing.
#[tauri::command]
pub fn set_parallax_enabled(enabled: bool) -> Result<ParallaxConfig, String> {
    let (path, mut doc) = read_config_doc()?;

    if enabled {
        let current_scale = doc.get("scale").and_then(|v| v.as_float()).unwrap_or(1.0);
        if current_scale <= 1.001 {
            doc["scale"] = toml_edit::value(1.10);
        }

        if let Some(anim) = doc
            .get_mut("animations")
            .and_then(|i| i.as_table_like_mut())
        {
            if let Some(parallax) = anim.get_mut("parallax").and_then(|i| i.as_table_like_mut()) {
                parallax.remove("off");
                if !parallax.contains_key("duration_ms") {
                    parallax.insert("duration_ms", toml_edit::value(600));
                }
                if !parallax.contains_key("curve") {
                    parallax.insert("curve", toml_edit::value("ease-out-cubic"));
                }
            }
            if anim.get("off").and_then(|v| v.as_bool()) == Some(true) {
                anim.remove("off");
            }
        }
    } else {
        doc["scale"] = toml_edit::value(1.0);

        if !doc.contains_key("animations") {
            doc["animations"] = Item::Table(toml_edit::Table::new());
        }
        if let Some(anim) = doc
            .get_mut("animations")
            .and_then(|i| i.as_table_like_mut())
        {
            if !anim.contains_key("parallax") {
                anim.insert("parallax", Item::Table(toml_edit::Table::new()));
            }
            if let Some(parallax) = anim.get_mut("parallax").and_then(|i| i.as_table_like_mut()) {
                // §2.17: `off` 不能与 duration_ms / curve 共存，写 off 前必须移除
                parallax.remove("duration_ms");
                parallax.remove("curve");
                parallax.remove("spring");
                parallax.remove("cubic_bezier");
                parallax.insert("off", toml_edit::value(true));
            }
        }
    }

    write_config_atomic(&path, &doc)?;
    get_parallax_config()
}

/// Save all parallax parameters directly to `~/.config/niripaper/config.toml`.
#[tauri::command]
pub fn save_parallax_config(cfg: ParallaxConfig) -> Result<String, String> {
    let (path, mut doc) = read_config_doc()?;

    if cfg.enabled {
        doc["scale"] = toml_edit::value(cfg.scale.clamp(1.0, 1.35));
    } else {
        doc["scale"] = toml_edit::value(1.0);
    }

    doc["column_span"] = toml_edit::value(cfg.column_span as i64);
    doc["workspace_span"] = toml_edit::value(cfg.workspace_span as i64);

    if !doc.contains_key("animations") {
        doc["animations"] = Item::Table(toml_edit::Table::new());
    }
    if let Some(anim) = doc
        .get_mut("animations")
        .and_then(|i| i.as_table_like_mut())
    {
        anim.insert("follow_niri", toml_edit::value(cfg.follow_niri));

        if !anim.contains_key("parallax") {
            anim.insert("parallax", Item::Table(toml_edit::Table::new()));
        }
        if let Some(parallax) = anim.get_mut("parallax").and_then(|i| i.as_table_like_mut()) {
            if cfg.enabled {
                parallax.remove("off");
                parallax.insert("duration_ms", toml_edit::value(cfg.duration_ms as i64));
                parallax.insert("curve", toml_edit::value(cfg.curve));
            } else {
                parallax.remove("duration_ms");
                parallax.remove("curve");
                parallax.remove("spring");
                parallax.remove("cubic_bezier");
                parallax.insert("off", toml_edit::value(true));
            }
        }

        if !anim.contains_key("overview-open-close") {
            anim.insert("overview-open-close", Item::Table(toml_edit::Table::new()));
        }
        if let Some(overview) = anim
            .get_mut("overview-open-close")
            .and_then(|i| i.as_table_like_mut())
        {
            overview.insert("zoom", toml_edit::value(cfg.overview_zoom.clamp(0.5, 1.0)));
        }
    }

    write_config_atomic(&path, &doc)?;
    Ok("视差与动画参数已写入 config.toml 并即时生效".to_owned())
}
