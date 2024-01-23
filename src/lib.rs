use std::{path::PathBuf, sync::Arc, time::Duration};

use aggregator::AggMessage;
use file::FileFormat;
use futures_util::StreamExt;
use rust_decimal::Decimal;
use tokio::{sync::mpsc, task::JoinSet};

mod aggregator;
mod bybite;
mod file;

pub use file::read;

pub async fn cache(time_in_sec: u64, file: PathBuf, clients: usize) {
    let mut tasks = JoinSet::new();
    let barrier = Arc::new(tokio::sync::Barrier::new(clients));
    let (tx_file, rx_file) = mpsc::channel(clients);
    let (tx_agg, rx_agg) = mpsc::channel(clients);

    tracing::debug!("Generating keys");

    let (signing_key, verifying_key) = aggregator::key_gen();

    tasks.spawn(async move {
        file::file_writer(file, rx_file).await.unwrap();
    });

    let tx_file_clone = tx_file.clone();
    tasks.spawn(async move {
        aggregator::aggregator(rx_agg, tx_file_clone, verifying_key)
            .await
            .unwrap();
    });

    for i in 0..clients {
        let barrier = barrier.clone();
        let tx_file = tx_file.clone();
        let tx_agg = tx_agg.clone();
        let signing_key = signing_key.clone();

        tasks.spawn(async move {
            client(i, time_in_sec, barrier, tx_file, tx_agg, signing_key)
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

async fn client(
    id: usize,
    time_in_sec: u64,
    barrier: Arc<tokio::sync::Barrier>,
    tx_file: mpsc::Sender<FileFormat>,
    tx_agg: mpsc::Sender<AggMessage>,
    sign_key: aggregator::SigningKey,
) -> anyhow::Result<()> {
    let tx_file_ref = &tx_file;
    let ws = bybite::bybite_ws(barrier).await?;

    let (sum, count) = ws
        .take_until(tokio::time::sleep(Duration::from_secs(time_in_sec)))
        .filter_map(|msg| std::future::ready(msg.ok()))
        .filter_map(|msg| std::future::ready(bybite::TickerResponse::try_from(msg).ok()))
        .map(|x| x.price())
        .filter_map(|x| async move {
            tx_file_ref
                .send(FileFormat::data_point(id, x))
                .await
                .ok()
                .map(|_| x)
        })
        .fold((Decimal::ZERO, 0), |(sum, count), x| {
            std::future::ready((sum + x, count + 1))
        })
        .await;

    let avg = sum / Decimal::from(count);

    tx_agg.send(AggMessage::with_key(avg, sign_key)).await?;
    tx_file.send(FileFormat::client_average(id, avg)).await?;

    Ok(())
}