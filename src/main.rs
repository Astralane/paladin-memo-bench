use crate::slot_stream::create_palidator_slot_stream;
use futures_util::StreamExt;
use solana_client::nonblocking::pubsub_client::PubsubClient;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

mod memo_tx;
mod slot_stream;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pub_sub = PubsubClient::new("ws://rpc:8900").await?;
    let resp = reqwest::Client::new()
        .request(
            reqwest::Method::GET,
            "http://paris.gateway.astralane.io/api/palidators",
        )
        .send()
        .await?;
    let body = resp.text().await?;

    let schedule_map = serde_json::from_str::<HashMap<String, Vec<u64>>>(&body)?;

    let schedule = schedule_map
        .values()
        .into_iter()
        .flatten()
        .cloned()
        .collect();
    let mut stream = create_palidator_slot_stream(&pub_sub, Arc::new(schedule)).await?;
    while let Some(slot) = stream.next().await {
        println!("stream got {:?}", slot);
    }

    Ok(())
}
