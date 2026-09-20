use geoscribefs::{client::{ClientCommand, run_client}, server::run_server};
use std::time::Duration;
use tokio::time::sleep;
use tonic::transport::Endpoint;

async fn wait_for_server(addr: &str) {
    let mut attempts = 0;
    let max_attempts = 10;

    while attempts < max_attempts {
        // Attempt to connect to the address
        if Endpoint::from_shared(format!("http://{}", addr))
            .unwrap()
            .connect()
            .await
            .is_ok()
        {
            return; // Server is up!
        }

        attempts += 1;
        sleep(Duration::from_millis(100)).await;
    }
    panic!("Server at {} failed to start in time", addr);
}

#[cfg(test)]
use pretty_assertions::{assert_eq};

#[tokio::test]
async fn test_status() {
    let addr = "127.0.0.1:50051";
    let addr2 = "127.0.0.1:50052";
    let addr3 = "127.0.0.1:50053";
    let token = "secret_token".to_string();

    let token1 = token.clone();
    tokio::spawn(async move {
        run_server(addr, token1, vec![addr2.to_string(), addr3.to_string()]).await.unwrap();
    });

    let token2 = token.clone();
    tokio::spawn(async move {
        run_server(addr2, token2, vec![addr.to_string(), addr3.to_string()]).await.unwrap();
    });

    let token3 = token.clone();
    tokio::spawn(async move {
        run_server(addr3, token3, vec![addr.to_string(), addr2.to_string()]).await.unwrap();
    });

    wait_for_server(addr).await;

    run_client(addr, ClientCommand::Write, "secret_token".to_string(), Some("vol_a".to_string())).await.unwrap();
    run_client("127.0.0.1:50052", ClientCommand::Write, "secret_token".to_string(), Some("vol_b".to_string())).await.unwrap();

    let result = run_client(addr, ClientCommand::Status, "secret_token".to_string(), None).await.unwrap();
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
