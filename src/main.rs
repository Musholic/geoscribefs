use clap::{Parser, Subcommand};
use geoscribefs::{DEFAULT_ADDR, TOKEN_ENV_VAR};

mod client;
mod server;

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
    Start,
    Status,
    Stop,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let addr = cli.addr;

    match cli.command {
        Commands::Start => {
            server::run_server(&addr, cli.token.clone()).await?;
        }
        Commands::Status => {
            client::run_client(&addr, "status", cli.token.clone()).await?;
        }
        Commands::Stop => {
            client::run_client(&addr, "stop", cli.token.clone()).await?;
        }
    }

    Ok(())
}
