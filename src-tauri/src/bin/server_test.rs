/// Standalone backend server (no Tauri window).
/// Usage: cargo run --bin meem-server --manifest-path src-tauri/Cargo.toml
use app_lib::server;

#[tokio::main]
async fn main() {
    let _guard = server::init_tracing();
    if let Err(e) = server::start_server().await {
        eprintln!("Backend server error: {}", e);
        std::process::exit(1);
    }
}
