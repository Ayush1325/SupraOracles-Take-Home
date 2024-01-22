use std::{path::PathBuf, time::Duration};

use clap::{Parser, ValueEnum};
use futures_util::{pin_mut, SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

#[derive(Parser)]
struct Cli {
    #[clap(long)]
    mode: Mode,
    #[clap(long)]
    times: u64,
    #[clap(default_value = "data.json")]
    file: PathBuf,
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

#[derive(Deserialize, Serialize, Debug, Clone)]
struct TickerResponse {
    topic: String,
    #[serde(with = "chrono::serde::ts_milliseconds")]
    ts: chrono::DateTime<chrono::Utc>,
    cs: u64,
    data: TickerData,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct TickerData {
    symbol: String,
    last_price: Decimal,
    usd_index_price: Decimal,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let (tx, mut rx) = tokio::sync::broadcast::channel::<TickerResponse>(10);

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

    let mut rx1 = tx.subscribe();
    let compute_task = tokio::spawn(async move {
        let mut avg = Decimal::new(0, 0);
        let mut count = 0;
        while let Ok(msg) = rx1.recv().await {
            avg += msg.data.last_price;
            count += 1;
            tracing::info!("Receive: {:?}", msg);
        }

        avg /= Decimal::from(count);
        tracing::info!("The average USD price of BTC is: {avg}");
    });

    let file_task = tokio::spawn(async move {
        let mut file = tokio::fs::File::create("data.json")
            .await
            .expect("Failed to create file");

        while let Ok(msg) = rx.recv().await {
            let data = serde_json::to_vec(&msg).expect("Failed to serialize response");
            file.write(&data).await.expect("Failed to write file");
            file.write(b"\n").await.expect("Failed to write file");
        }
    });

    let task = tokio::spawn(async move {
        tracing::info!("Start");

        let stream = ws
            .filter_map(|msg| async { msg.ok() })
            .filter_map(|msg| async {
                match msg {
                    tokio_tungstenite::tungstenite::Message::Text(text) => {
                        serde_json::from_str::<TickerResponse>(&text).ok()
                    }
                    _ => None,
                }
            });

        pin_mut!(stream);
        while let Some(x) = stream.next().await {
            tx.send(x).expect("Failed to broadcast");
        }
    });

    tokio::time::sleep(Duration::from_secs(10)).await;

    task.abort();

    let (first, second) = tokio::join!(file_task, compute_task);

    tracing::info!("Finish");
}
