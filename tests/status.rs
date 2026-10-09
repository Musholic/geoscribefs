mod common;

use geoscribefs::{client::GeoScribeClient, server::ServerConfig};

#[cfg(test)]
use pretty_assertions::assert_eq;

use crate::common::{get_random_addr, start_server};

#[tokio::test]
async fn test_status() -> Result<(), Box<dyn std::error::Error>> {
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

    let mut client =
        GeoScribeClient::connect(addr.to_string(), ServerConfig::default().token).await?;
    client.write("vol_a".to_string()).await?;

    let mut client2 =
        GeoScribeClient::connect(addr2.to_string(), ServerConfig::default().token).await?;
    client2.write("vol_b".to_string()).await?;

    let result = client.status().await?;

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

    Ok(())
}
