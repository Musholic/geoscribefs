use clap::{Parser, Subcommand};
use geoscribefs::{DEFAULT_ADDR, TOKEN_ENV_VAR, client, server};
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
        volumes: Vec<String>,
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
        Commands::Start { peers, volumes } => {
            server::run_server(server::ServerConfig {
                addr,
                token,
                peers,
                volumes,
            })
            .await?;
        }
        Commands::Status => {
            let result =
                client::run_client(addr, client::ClientCommand::Status, token, None).await?;
            println!("{}", result);
        }
        Commands::Write { volume } => {
            let result =
                client::run_client(addr, client::ClientCommand::Write, token, Some(volume)).await?;
            println!("{}", result);
        }
    }

    Ok(())
}
