pub mod server;

use tauri::{Manager, Emitter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing BEFORE Tauri (which sets its own logger).
    // The guard must live for the entire process — dropping it loses buffered logs.
    let _log_guard = server::init_tracing();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_deep_link::init());

    // single-instance + deep-link: if a second copy launches with a meem:// URL,
    // forward it to the running instance instead of starting a new one.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        for arg in &args {
            if arg.starts_with("meem://") {
                let _ = app.emit("deep-link://new-url", vec![arg.clone()]);
            }
        }
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));

    builder
        .setup(|app| {
            // Register the meem:// scheme with the OS at runtime. The bundle installer
            // does this on production installs; dev builds need to call it explicitly
            // or the browser has nothing to hand `meem://...` URLs to.
            #[cfg(any(windows, target_os = "linux"))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                if let Err(e) = app.deep_link().register_all() {
                    tracing::warn!("Failed to register deep-link schemes: {}", e);
                }
            }

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

