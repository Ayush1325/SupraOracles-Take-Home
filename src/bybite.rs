use std::sync::Arc;

use futures_util::SinkExt;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite;

const URL: &str = "wss://stream.bybit.com/v5/public/spot";

#[derive(Serialize)]
struct SubscriptionRequest {
    op: &'static str,
    args: &'static [&'static str],
}

impl SubscriptionRequest {
    const fn new(args: &'static [&'static str]) -> Self {
        Self {
            op: "subscribe",
            args,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct TickerResponse {
    data: TickerData,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TickerData {
    last_price: Decimal,
}

impl TickerResponse {
    pub const fn price(&self) -> Decimal {
        self.data.last_price
    }
}

impl TryFrom<tungstenite::Message> for TickerResponse {
    type Error = anyhow::Error;

    fn try_from(msg: tungstenite::Message) -> Result<Self, Self::Error> {
        match msg {
            tungstenite::Message::Text(text) => serde_json::from_str(&text).map_err(Into::into),
            _ => Err(anyhow::anyhow!("Invalid message type")),
        }
    }
}

pub async fn bybite_ws(
    barrier: Arc<tokio::sync::Barrier>,
) -> anyhow::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let (mut ws, _resp) = tokio_tungstenite::connect_async(URL).await?;
    let req = SubscriptionRequest::new(&["tickers.BTCUSDT"]);

    barrier.wait().await;

    ws.send(tungstenite::Message::Text(serde_json::to_string(&req)?))
        .await?;

    Ok(ws)
}
