pub mod server;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing BEFORE Tauri (which sets its own logger).
    // The guard must live for the entire process — dropping it loses buffered logs.
    let _log_guard = server::init_tracing();

    tauri::Builder::default()
        .setup(|app| {
            // Spawn the embedded backend server
            tauri::async_runtime::spawn(async {
                if let Err(e) = server::start_server().await {
                    tracing::error!("Backend server error: {}", e);
                    eprintln!("Backend server error: {}", e);
                }
            });

            // Resize window to 75% of the current monitor
            if let Some(window) = app.get_webview_window("main") {
                if let Some(monitor) = window.current_monitor().ok().flatten() {
                    let size = monitor.size();
                    let width = (size.width as f64 * 0.75) as u32;
                    let height = (size.height as f64 * 0.75) as u32;
                    let _ = window.set_size(tauri::Size::Physical(
                        tauri::PhysicalSize { width, height },
                    ));
                    let _ = window.center();
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
