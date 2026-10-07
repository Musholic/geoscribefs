mod common;

use geoscribefs::{
    client::{ClientCommand, run_client},
    server::ServerConfig,
};

#[cfg(test)]
use pretty_assertions::assert_eq;

use crate::common::start_server;

#[tokio::test]
async fn test_status() {
    let addr = "127.0.0.1:50051";
    let addr2 = "127.0.0.1:50052";
    let addr3 = "127.0.0.1:50053";

    start_server(ServerConfig {
        addr: addr.to_string(),
        peers: vec![addr2.to_string(), addr3.to_string()],
        ..Default::default()
    })
    .await;
    start_server(ServerConfig {
        addr: addr2.to_string(),
        peers: vec![addr.to_string(), addr3.to_string()],
        ..Default::default()
    })
    .await;

    start_server(ServerConfig {
        addr: addr3.to_string(),
        peers: vec![addr.to_string(), addr2.to_string()],
        ..Default::default()
    })
    .await;

    run_client(
        addr.to_string(),
        ClientCommand::Write,
        ServerConfig::default().token,
        Some("vol_a".to_string()),
    )
    .await
    .unwrap();

    run_client(
        addr2.to_string(),
        ClientCommand::Write,
        ServerConfig::default().token,
        Some("vol_b".to_string()),
    )
    .await
    .unwrap();

    let result = run_client(
        addr.to_string(),
        ClientCommand::Status,
        ServerConfig::default().token,
        None,
    )
    .await
    .unwrap();

    let expected = "\
Addr: 127.0.0.1:50051
Connected to:
- 127.0.0.1:50052
- 127.0.0.1:50053
Volumes:
- vol_a: 127.0.0.1:50051
- vol_b: 127.0.0.1:50052
";
    assert_eq!(result, expected);
}
