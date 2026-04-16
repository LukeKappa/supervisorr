mod config;
mod daemon;
mod client;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "supervisorr")]
#[command(about = "A zero-dependency process manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Starts the supervisor daemon
    Daemon {
        #[arg(short, long, default_value = "/etc/supervisorr/supervisorr.toml")]
        config: String,
    },
    /// Status of processes
    Status,
    /// Start a process
    Start { target: String },
    /// Stop a process
    Stop { target: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Daemon { config } => daemon::run(config).await,
        Commands::Status => client::status().await,
        Commands::Start { target } => client::start(target).await,
        Commands::Stop { target } => client::stop(target).await,
    }
}
