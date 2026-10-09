mod common;

use geoscribefs::{
    client::{ClientCommand, run_client},
    server::ServerConfig,
};

#[cfg(test)]
use pretty_assertions::assert_eq;

use crate::common::{get_random_addr, start_server};

#[tokio::test]
async fn test_status() {
    let addr = get_random_addr();
    let addr2 = get_random_addr();
    let addr3 = get_random_addr();

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

    let expected = format!(
        "Addr: {addr}\n\
Connected to:\n\
- {addr2}\n\
- {addr3}\n\
Volumes:
- vol_a: {addr}\n\
- vol_b: {addr2}\n"
    );
    assert_eq!(result, expected);
}
