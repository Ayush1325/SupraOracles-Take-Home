use std::{path::PathBuf, time::Duration};

use clap::{Parser, ValueEnum};
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio_tungstenite::tungstenite;

const URL: &str = "wss://stream.bybit.com/v5/public/spot";

#[derive(Parser)]
struct Cli {
    #[clap(long)]
    mode: Mode,
    #[clap(long, default_value = "10")]
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

#[derive(Deserialize, Serialize, Debug, Clone)]
enum FileFormat {
    DataPoint(Decimal),
    Average(Decimal),
}

impl std::fmt::Display for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileFormat::DataPoint(x) => write!(f, "Data Point: {x}"),
            FileFormat::Average(x) => write!(f, "Average: {x}"),
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.mode {
        Mode::Cache => cache(cli.times, cli.file).await,
        Mode::Read => tokio::task::spawn_blocking(|| read(cli.file))
            .await
            .expect("Failed to read"),
    }
}

fn read(file: PathBuf) {
    let file = std::fs::File::open(file).expect("Failed to open file");

    serde_json::Deserializer::from_reader(file)
        .into_iter::<FileFormat>()
        .for_each(|x| {
            let x = x.expect("Failed to deserialize");
            println!("{x}");
        });
}

async fn cache(time_in_sec: u64, file: PathBuf) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<TickerResponse>(10);

    let compute_task = tokio::spawn(async move {
        let mut file = tokio::fs::File::create(file)
            .await
            .expect("Failed to create file");
        let mut avg = Decimal::new(0, 0);
        let mut count = 0;

        while let Some(msg) = rx.recv().await {
            avg += msg.data.last_price;
            count += 1;

            let data = serde_json::to_vec(&FileFormat::DataPoint(msg.data.last_price)).unwrap();
            file.write(&data).await.expect("Failed to write file");
            file.write(b"\n").await.expect("Failed to write file");
        }

        avg /= Decimal::from(count);
        println!("Cache Complete. The average USD price of BTC is: {avg}");

        let data = serde_json::to_vec(&FileFormat::Average(avg)).unwrap();
        file.write(&data).await.expect("Failed to write file");
    });

    let task = tokio::spawn(async move {
        tracing::debug!("Start");

        let ws = bybite_ws().await.expect("Failed to connect to bybite");
        let tx = tokio_util::sync::PollSender::new(tx);
        let _stream = ws
            .filter_map(|msg| std::future::ready(msg.ok()))
            .filter_map(|msg| async {
                match msg {
                    tungstenite::Message::Text(text) => {
                        serde_json::from_str::<TickerResponse>(&text).ok()
                    }
                    _ => None,
                }
            })
            .map(Ok)
            .forward(tx)
            .await;
    });
    tokio::time::sleep(Duration::from_secs(time_in_sec)).await;
    task.abort();

    compute_task.await.expect("Failed to compute");
}

async fn bybite_ws() -> anyhow::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let (mut ws, _resp) = tokio_tungstenite::connect_async(URL).await?;

    let req = SubscriptionRequest {
        op: "subscribe".to_string(),
        args: vec!["tickers.BTCUSDT".to_string()],
    };

    ws.send(tungstenite::Message::Text(serde_json::to_string(&req)?))
        .await?;

    Ok(ws)
}
