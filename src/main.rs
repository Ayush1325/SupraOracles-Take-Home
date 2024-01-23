use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Parser)]
struct Cli {
    #[clap(long)]
    mode: Mode,
    #[clap(long, default_value = "10")]
    times: u64,
    #[clap(long, default_value = "data.json")]
    file: PathBuf,
    #[clap(long, default_value = "5")]
    clients: usize,
}

#[derive(ValueEnum, Clone, Debug)]
enum Mode {
    Cache,
    Read,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.mode {
        Mode::Cache => simple::cache(cli.times, cli.file, cli.clients).await,
        Mode::Read => tokio::task::spawn_blocking(|| simple::read(cli.file))
            .await
            .expect("Failed to read"),
    }
}
