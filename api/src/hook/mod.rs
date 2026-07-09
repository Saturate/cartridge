pub mod server;
pub mod socket;

use std::io::Read;

pub fn run(event: &str) {
    let agent_id = std::env::var("CARTRIDGE_AGENT_ID").unwrap_or_default();
    let socket_path = std::env::var("CARTRIDGE_API_SOCKET")
        .unwrap_or_else(|_| "/tmp/cartridge-api.sock".into());

    let mut payload = String::new();
    std::io::stdin().read_to_string(&mut payload).ok();

    let envelope = serde_json::json!({
        "agent_id": agent_id,
        "event": event,
        "payload": payload,
    });

    let msg = serde_json::to_vec(&envelope).unwrap_or_default();
    if let Err(e) = socket::send(&socket_path, &msg) {
        eprintln!("cartridge-api hook: {e}");
        std::process::exit(1);
    }
}

pub fn probe_socket(path: &str) -> bool {
    std::os::unix::net::UnixStream::connect(path).is_ok()
}
