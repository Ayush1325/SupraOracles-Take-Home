use std::{path::PathBuf, time::Duration};

use clap::{Parser, ValueEnum};
use futures_util::{pin_mut, SinkExt, StreamExt};
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
    #[clap(default_value = "5")]
    clients: usize,
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
    ts: u64,
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
        Mode::Cache => cache(cli.times, cli.file, cli.clients).await,
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

async fn cache(time_in_sec: u64, file: PathBuf, clients: usize) {
    client(time_in_sec, file).await.unwrap();
}

async fn client(time_in_sec: u64, file: PathBuf) -> anyhow::Result<()> {
    let mut file = tokio::fs::File::create(file).await?;

    let mut avg = Decimal::new(0, 0);
    let mut count = 0;

    let ws = bybite_ws().await?;
    let stream = ws
        .take_until(tokio::time::sleep(Duration::from_secs(time_in_sec)))
        .filter_map(|msg| std::future::ready(msg.ok()))
        .filter_map(|msg| async {
            match msg {
                tungstenite::Message::Text(text) => {
                    serde_json::from_str::<TickerResponse>(&text).ok()
                }
                _ => None,
            }
        })
        .map(|x| x.data.last_price);

    pin_mut!(stream);
    while let Some(msg) = stream.next().await {
        avg += msg;
        count += 1;

        let data = serde_json::to_vec(&FileFormat::DataPoint(msg))?;
        file.write(&data).await?;
        file.write(b"\n").await?;
    }

    avg /= Decimal::from(count);
    println!("Cache Complete. The average USD price of BTC is: {avg}");

    let data = serde_json::to_vec(&FileFormat::Average(avg))?;
    file.write(&data).await?;

    Ok(())
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
