use std::time::Duration;

use anyhow::{Context, Result};
use redis::streams::{StreamReadOptions, StreamReadReply};
use redis::AsyncCommands;

use crate::book::{match_order, Book};
use crate::redis_io::{Redis, STREAM_ORDERS};
use crate::types::Order;

const IDLE_SLEEP: Duration = Duration::from_millis(50);

pub async fn run(redis_url: String) -> Result<()> {
    let redis = Redis::connect(&redis_url).await?;
    let client = redis::Client::open(redis_url.as_str())?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    let mut book = Book::new();
    let mut last_id = "0".to_string();
    let opts = StreamReadOptions::default().count(128);

    tracing::info!("matcher started");

    loop {
        let reply: StreamReadReply = conn
            .xread_options(&[STREAM_ORDERS], &[&last_id], &opts)
            .await
            .context("xread orders")?;

        let mut processed = 0usize;
        for key in reply.keys {
            for entry in key.ids {
                last_id = entry.id.clone();
                let Some(data) = entry.map.get("data") else { continue };
                let payload: String = redis::from_redis_value(data.clone())?;
                let order: Order = serde_json::from_str(&payload)
                    .context("decode order")?;

                let fills = match_order(&mut book, order);
                for fill in &fills {
                    redis.publish_fill(fill).await?;
                }
                processed += 1;
            }
        }

        if processed > 0 {
            redis.set_snapshot(&book.snapshot()).await?;
        } else {
            tokio::time::sleep(IDLE_SLEEP).await;
        }
    }
}
