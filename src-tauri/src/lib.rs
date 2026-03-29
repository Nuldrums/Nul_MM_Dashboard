use tauri::Manager;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;
use std::sync::Mutex;

/// Holds the sidecar process handle for cleanup on exit.
struct SidecarState(Mutex<Option<CommandChild>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Spawn the Python backend sidecar
            let shell = app.shell();
            let sidecar = shell
                .sidecar("marketing-engine-backend")
                .expect("failed to create sidecar command");

            let (mut _rx, child) = sidecar.spawn().expect("failed to spawn sidecar");

            // Store the child handle so we can kill it on exit
            app.manage(SidecarState(Mutex::new(Some(child))));

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // Kill the sidecar when the window closes
                if let Some(state) = window.try_state::<SidecarState>() {
                    if let Ok(mut guard) = state.0.lock() {
                        if let Some(child) = guard.take() {
                            let _ = child.kill();
                        }
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
