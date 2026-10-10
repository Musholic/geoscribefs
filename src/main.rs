use clap::Parser;
use geoscribefs::cli::{Cli, run_cli};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let result = run_cli(Cli::parse()).await?;
    println!("{}", result);

    Ok(())
}
