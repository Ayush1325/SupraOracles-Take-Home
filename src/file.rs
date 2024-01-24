//! File related utilities

use std::path::PathBuf;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, sync::mpsc};

#[derive(Deserialize, Serialize, Debug, Clone)]
/// Format to use when writing to cache file
pub enum FileFormat {
    DataPoint { client_id: usize, price: Decimal },
    ClientAverage { client_id: usize, avg: Decimal },
    AggAvg(Decimal),
}

impl FileFormat {
    /// Helper to create a data point
    pub const fn data_point(client_id: usize, price: Decimal) -> Self {
        Self::DataPoint { client_id, price }
    }

    /// Helper to create a client average
    pub const fn client_average(client_id: usize, avg: Decimal) -> Self {
        Self::ClientAverage { client_id, avg }
    }

    /// Helper to create an aggregator average
    pub const fn agg_avg(avg: Decimal) -> Self {
        Self::AggAvg(avg)
    }
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

/// Parse data from cache file
pub fn read(file: PathBuf) -> impl Iterator<Item = serde_json::Result<FileFormat>> {
    let file = std::fs::File::open(file).expect("Failed to open file");

    serde_json::Deserializer::from_reader(file).into_iter::<FileFormat>()
}

/// Write data to cache file from a channel
pub async fn file_writer(file: PathBuf, mut rx: mpsc::Receiver<FileFormat>) -> anyhow::Result<()> {
    tracing::debug!("Opening file: {:?}", file);
    let mut file = tokio::fs::File::create(file).await?;

    while let Some(msg) = rx.recv().await {
        let data = serde_json::to_vec(&msg)?;
        file.write(&data).await?;
        file.write(b"\n").await?;
    }

    Ok(())
}
