use tauri::Manager;
use std::sync::Mutex;

/// Holds the sidecar process handle for cleanup on exit.
struct SidecarChild(Mutex<Option<std::process::Child>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Configure logging in debug builds
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Spawn the Python backend sidecar on startup.
            // In dev mode, we assume the developer runs uvicorn manually,
            // so we only launch the sidecar in release builds.
            if !cfg!(debug_assertions) {
                // Resolve the sidecar binary path relative to the app's resource dir.
                // Tauri places externalBin entries next to the main executable.
                let resource_dir = app
                    .path()
                    .resource_dir()
                    .expect("failed to resolve resource directory");

                let sidecar_name = if cfg!(target_os = "windows") {
                    "marketing-engine-backend.exe"
                } else {
                    "marketing-engine-backend"
                };

                let sidecar_path = resource_dir.join(sidecar_name);

                match std::process::Command::new(&sidecar_path).spawn() {
                    Ok(child) => {
                        log::info!("Sidecar launched: {:?}", sidecar_path);
                        app.manage(SidecarChild(Mutex::new(Some(child))));
                    }
                    Err(e) => {
                        log::error!("Failed to spawn sidecar at {:?}: {}", sidecar_path, e);
                        // Non-fatal: the user can start the backend manually
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // Kill the sidecar when the window is destroyed
                if let Some(state) = window.try_state::<SidecarChild>() {
                    if let Ok(mut guard) = state.0.lock() {
                        if let Some(mut child) = guard.take() {
                            log::info!("Killing sidecar process");
                            let _ = child.kill();
                            let _ = child.wait();
                        }
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
