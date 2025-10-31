use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

use chrono::Utc;
use dora_protocol::DataflowStatus;
use dora_protocol_client::ProtocolClients;
use uuid::Uuid;

#[test]
fn protocol_client_ignores_proxy_environment() {
    // Spin up a tiny HTTP server that serves a deterministic response.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0u8; 1024];
            let _ = stream.read(&mut buffer);

            let body = serde_json::json!([{
                "id": Uuid::new_v4(),
                "name": "demo",
                "status": DataflowStatus::Running,
                "updated_at": Utc::now(),
                "nodes": []
            }])
            .to_string();

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
    });

    // Set proxy environment variables to invalid values. If reqwest respected them,
    // the request would fail before reaching our local server.
    unsafe {
        std::env::set_var("HTTP_PROXY", "http://127.0.0.1:9");
        std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:9");
        std::env::set_var("ALL_PROXY", "http://127.0.0.1:9");
    }

    let base_url = format!("http://{addr}");
    let clients = ProtocolClients::new(&base_url).expect("client construction");
    let summaries = clients
        .coordinator_client()
        .list_dataflows()
        .expect("list dataflows");

    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].name, "demo");
    assert_eq!(summaries[0].status, "running");

    unsafe {
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("HTTPS_PROXY");
        std::env::remove_var("ALL_PROXY");
    }

    server.join().unwrap();
}
