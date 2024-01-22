use clap::{Parser, ValueEnum};

#[derive(Parser)]
struct Cli {
    #[clap(long)]
    mode: Mode,
    #[clap(long)]
    times: u64
}

#[derive(ValueEnum, Clone, Debug)]
enum Mode {
    Cache,
    Read
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Hello, world!");
}
