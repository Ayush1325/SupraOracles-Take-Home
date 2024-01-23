use std::{path::PathBuf, sync::Arc, time::Duration};

use clap::{Parser, ValueEnum};
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, sync::mpsc, task::JoinSet};
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
    DataPoint { client_id: usize, price: Decimal },
    ClientAverage { client_id: usize, avg: Decimal },
    AggAvg(Decimal),
}

impl std::fmt::Display for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileFormat::DataPoint { client_id, price } => {
                write!(f, "Data Point: client_id: {client_id}, price: {price}")
            }
            FileFormat::ClientAverage { client_id, avg } => {
                write!(f, "Client Average: client_id: {client_id}, avg: {avg}")
            }
            FileFormat::AggAvg(x) => write!(f, "Aggregator Average: {x}"),
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
    let mut tasks = JoinSet::new();
    let barrier = Arc::new(tokio::sync::Barrier::new(clients));
    let (tx_file, rx_file) = mpsc::channel(clients);
    let (tx_agg, rx_agg) = mpsc::channel(clients);

    tasks.spawn(async move {
        file_writer(file, rx_file).await.unwrap();
    });

    let tx_file_clone = tx_file.clone();
    tasks.spawn(async move {
        aggregator(rx_agg, tx_file_clone).await.unwrap();
    });

    for i in 0..clients {
        let barrier = barrier.clone();
        let tx_file = tx_file.clone();
        let tx_agg = tx_agg.clone();
        tasks.spawn(async move {
            client(i, time_in_sec, barrier, tx_file, tx_agg)
                .await
                .unwrap();
        });
    }

    drop(tx_agg);
    drop(tx_file);

    while let Some(res) = tasks.join_next().await {
        res.unwrap();
    }
}

async fn aggregator(
    mut rx: mpsc::Receiver<Decimal>,
    tx_file: mpsc::Sender<FileFormat>,
) -> anyhow::Result<()> {
    let mut avg = Decimal::new(0, 0);
    let mut count = 0;

    while let Some(msg) = rx.recv().await {
        avg += msg;
        count += 1;
    }

    avg /= Decimal::from(count);
    println!("Cache Complete. The average USD price of BTC is: {avg}");

    tx_file
        .send(FileFormat::AggAvg(avg))
        .await
        .map_err(Into::into)
}

async fn file_writer(file: PathBuf, mut rx: mpsc::Receiver<FileFormat>) -> anyhow::Result<()> {
    let mut file = tokio::fs::File::create(file).await?;

    while let Some(msg) = rx.recv().await {
        let data = serde_json::to_vec(&msg)?;
        file.write(&data).await?;
        file.write(b"\n").await?;
    }

    Ok(())
}

async fn client(
    id: usize,
    time_in_sec: u64,
    barrier: Arc<tokio::sync::Barrier>,
    tx_file: mpsc::Sender<FileFormat>,
    tx_agg: mpsc::Sender<Decimal>,
) -> anyhow::Result<()> {
    let tx_file_ref = &tx_file;
    let ws = bybite_ws(barrier).await?;

    let (sum, count) = ws
        .take_until(tokio::time::sleep(Duration::from_secs(time_in_sec)))
        .filter_map(|msg| std::future::ready(msg.ok()))
        .filter_map(|msg| async move {
            match msg {
                tungstenite::Message::Text(text) => {
                    serde_json::from_str::<TickerResponse>(&text).ok()
                }
                _ => None,
            }
        })
        .map(|x| x.data.last_price)
        .filter_map(|x| async move {
            tx_file_ref
                .send(FileFormat::DataPoint {
                    client_id: id,
                    price: x,
                })
                .await
                .ok()
                .map(|_| x)
        })
        .fold((Decimal::ZERO, 0), |(sum, count), x| {
            std::future::ready((sum + x, count + 1))
        })
        .await;

    let avg = sum / Decimal::from(count);

    tx_agg.send(avg).await?;
    tx_file
        .send(FileFormat::ClientAverage { client_id: id, avg })
        .await?;

    Ok(())
}

async fn bybite_ws(
    barrier: Arc<tokio::sync::Barrier>,
) -> anyhow::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let (mut ws, _resp) = tokio_tungstenite::connect_async(URL).await?;

    let req = SubscriptionRequest {
        op: "subscribe".to_string(),
        args: vec!["tickers.BTCUSDT".to_string()],
    };

    barrier.wait().await;

    ws.send(tungstenite::Message::Text(serde_json::to_string(&req)?))
        .await?;

    Ok(ws)
}
