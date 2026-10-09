use clap::{Parser, Subcommand};
use geoscribefs::{DEFAULT_ADDR, TOKEN_ENV_VAR, client::GeoScribeClient, server};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
struct Cli {
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
        #[arg(long)]
        volume: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let cli = Cli::parse();
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
        }
        Commands::Status => {
            let mut client = GeoScribeClient::connect(addr, token).await?;
            let result = client.status().await?;
            println!("{}", result);
        }
        Commands::Write { volume } => {
            let mut client = GeoScribeClient::connect(addr, token).await?;
            let result = client.write(volume).await?;
            println!("{}", result);
        }
    }

    Ok(())
}
