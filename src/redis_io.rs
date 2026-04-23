use anyhow::{Context, Result};
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;

use crate::types::{Fill, Order, Snapshot};

pub const STREAM_ORDERS: &str = "orders";
pub const CHANNEL_FILLS: &str = "fills";
pub const KEY_ORDERBOOK: &str = "orderbook";
pub const KEY_ORDER_SEQ: &str = "order:seq";

#[derive(Clone)]
pub struct Redis {
    conn: MultiplexedConnection,
    url: String,
}

impl Redis {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = redis::Client::open(url).context("open redis client")?;
        let conn = client
            .get_multiplexed_async_connection()
            .await
            .context("connect redis")?;
        Ok(Self {
            conn,
            url: url.to_string(),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn next_order_id(&self) -> Result<u64> {
        let mut c = self.conn.clone();
        let id: u64 = c.incr(KEY_ORDER_SEQ, 1).await?;
        Ok(id)
    }

    pub async fn xadd_order(&self, order: &Order) -> Result<String> {
        let mut c = self.conn.clone();
        let payload = serde_json::to_string(order)?;
        let id: String = c
            .xadd(STREAM_ORDERS, "*", &[("data", payload.as_str())])
            .await?;
        Ok(id)
    }

    pub async fn publish_fill(&self, fill: &Fill) -> Result<()> {
        let mut c = self.conn.clone();
        let payload = serde_json::to_string(fill)?;
        let _: () = c.publish(CHANNEL_FILLS, payload).await?;
        Ok(())
    }

    pub async fn set_snapshot(&self, snap: &Snapshot) -> Result<()> {
        let mut c = self.conn.clone();
        let payload = serde_json::to_string(snap)?;
        let _: () = c.set(KEY_ORDERBOOK, payload).await?;
        Ok(())
    }

    pub async fn get_snapshot(&self) -> Result<String> {
        let mut c = self.conn.clone();
        let s: Option<String> = c.get(KEY_ORDERBOOK).await?;
        Ok(s.unwrap_or_else(|| r#"{"bids":[],"asks":[]}"#.to_string()))
    }
}
