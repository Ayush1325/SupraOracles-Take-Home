use std::time::Duration;

use clap::{Parser, ValueEnum};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;

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

#[derive(Serialize)]
struct SubscriptionRequest {
    op: String,
    args: Vec<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let (mut ws, resp) = tokio_tungstenite::connect_async("wss://stream.bybit.com/v5/public/spot")
        .await
        .expect("Failed to connect Websocket");

    let req = SubscriptionRequest {
        op: "subscribe".to_string(),
        args: vec!["tickers.BTCUSDT".to_string()],
    };

    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        serde_json::to_string(&req).expect("Failed to serialize request"),
    ))
    .await
    .expect("Failed to send message");

    let task = tokio::spawn(async move {
        tracing::info!("Start");
        ws.for_each(|msg| async move {
            let msg = msg.expect("Failed to get message");
            tracing::info!("Receive: {:?}", msg);
        })
        .await;
    });

    tokio::time::sleep(Duration::from_secs(10)).await;

    task.abort();

    tracing::info!("Finish");
}
