// NiriPaper — the niripaper desktop manager (Tauri v2).
//
// The app is a thin client (PRD FR-1): it never decodes media, never links a
// decoder, never draws wallpaper pixels. Everything it shows comes from the
// daemon over the control socket (PRD FR-2), via `niripaper-proto`.
//
// Wiring (decisions §〇 + §4.2): one window plus a tray (US-014) offering
// exactly "open the main window" and "quit" — the tray never talks to the
// daemon.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // Tray: 打开主窗口 / 退出 (decisions §4.2). Everything else stays in
            // the main window.
            let open = MenuItem::with_id(app, "open", "打开主窗口", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().cloned().expect("app icon"))
                .tooltip("NiriPaper")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .manage(state::Connection::new())
        .invoke_handler(tauri::generate_handler![
            commands::daemon_schema,
            commands::daemon_state,
            commands::daemon_query,
            commands::daemon_set,
            commands::daemon_kill,
            commands::daemon_socket,
            commands::config_path,
        ])
        .run(tauri::generate_context!())
        .expect("启动 NiriPaper 失败");
}
