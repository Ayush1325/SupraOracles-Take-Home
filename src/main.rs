use std::time::Duration;

use clap::{Parser, ValueEnum};
use crypto_botters::{bybit::BybitOption, Client};

#[derive(Parser)]
struct Cli {
    #[clap(long)]
    mode: Mode,
    #[clap(long)]
    times: u64,
}

#[derive(ValueEnum, Clone, Debug)]
enum Mode {
    Cache,
    Read,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let client = Client::new();
    let connection = client
        .websocket(
            "/v5/public/spot",
            |message| tracing::info!("{}", message),
            [BybitOption::WebSocketTopics(vec![
                "tickers.BTCUSDT".to_string()
            ])],
        )
        .await
        .expect("Failed to connect to websocket");

    tokio::time::sleep(Duration::from_secs(10)).await;

    tracing::info!("Finish");
}
