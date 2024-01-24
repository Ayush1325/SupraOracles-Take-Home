use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[clap(long)]
    mode: Mode,
    #[clap(long, default_value = "10")]
    /// Time in seconds for reading from websocket
    times: u64,
    #[clap(long, default_value = "data.json")]
    /// Path to cache file
    file: PathBuf,
    #[clap(long, default_value = "5")]
    /// Number of clients to spawn
    clients: usize,
}

#[derive(ValueEnum, Clone, Debug)]
enum Mode {
    /// Fetch from websocket and cache
    Cache,
    /// Read from cache file
    Read,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    tracing::debug!("Parsing CLI");
    let cli = Cli::parse();

    match cli.mode {
        Mode::Cache => simple::cache(cli.times, cli.file, cli.clients).await,
        Mode::Read => simple::read(cli.file).await,
    }
}
