use clap::{Parser, Subcommand};

use crate::{client::GeoScribeClient, server};

pub const DEFAULT_ADDR: &str = "127.0.0.1:50051";
pub const TOKEN_ENV_VAR: &str = "GEOSCRIBE_TOKEN";

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, default_value = DEFAULT_ADDR)]
    addr: String,
    #[arg(long, env = TOKEN_ENV_VAR)]
    token: String,
}

#[derive(Subcommand)]
enum Commands {
    Start {
        #[arg(long)]
        peers: Vec<String>,
        #[arg(long)]
        base_volumes_path: String,
        #[arg(long)]
        volume_names: Vec<String>,
    },
    Status,
    Write {
        #[arg(short, long)]
        volume: String,
    },
    Snapshot {
        #[arg(short, long)]
        volume: String,
    },
}

pub async fn run_cli(cli: Cli) -> Result<String, Box<dyn std::error::Error>> {
    let addr = cli.addr;
    let token = cli.token;

    match cli.command {
        Commands::Start {
            peers,
            base_volumes_path,
            volume_names,
        } => {
            server::run_server(server::ServerConfig {
                addr,
                token,
                peers,
                base_volumes_path,
                volume_names,
            })
            .await?;
            Ok("Server exited".to_string())
        }
        Commands::Status => {
            let mut client = GeoScribeClient::connect(addr, token).await?;
            let result = client.status().await?;
            Ok(result.to_string())
        }
        Commands::Write { volume } => {
            let mut client = GeoScribeClient::connect(addr, token).await?;
            let result = client.write(volume).await?;
            Ok(result.to_string())
        }
        Commands::Snapshot { volume } => {
            let mut client = GeoScribeClient::connect(addr, token).await?;
            let (success, date) = client.snapshot(volume.clone()).await?;
            if success {
                Ok(format!(
                    "Snapshot created with success for {} with date: {}",
                    volume, date
                ))
            } else {
                Err("Snapshot creation failed".into())
            }
        }
    }
}
