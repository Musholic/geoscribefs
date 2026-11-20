use clap::Parser;
use geoscribefs::cli::{Cli, run_cli};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{EnvFilter, FmtSubscriber, fmt::format::FmtSpan};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Configure logging
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::NEW)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let result = run_cli(Cli::parse()).await?;
    println!("{}", result);

    Ok(())
}
