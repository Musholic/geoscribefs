use clap::{Parser, Subcommand};
use geoscribefs::{DEFAULT_ADDR, TOKEN_ENV_VAR, server, client};

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
    },
    Status,
    Write {
        #[arg(long)]
        volume: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let addr = cli.addr;

    match cli.command {
        Commands::Start { peers } => {
            server::run_server(&addr, cli.token.clone(), peers).await?;
        }
        Commands::Status => {
            let result = client::run_client(&addr, client::ClientCommand::Status, cli.token.clone(), None).await?;
            println!("{}", result);
        }
        Commands::Write { volume } => {
            let result = client::run_client(&addr, client::ClientCommand::Write, cli.token.clone(), Some(volume)).await?;
            println!("{}", result);
        }
    }

    Ok(())
}
