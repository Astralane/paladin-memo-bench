use crate::slot_stream::create_palidator_slot_stream;
use futures_util::StreamExt;
use solana_client::nonblocking::pubsub_client::PubsubClient;
use std::collections::HashSet;
use std::sync::Arc;

mod memo_tx;
mod slot_stream;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pub_sub = PubsubClient::new("ws://rpc:8900").await?;
    let schedule = Arc::new(HashSet::new());
    let mut stream = create_palidator_slot_stream(&pub_sub, schedule).await?;

    while let Some(slot) = stream.next().await {
        println!("{:?}", slot);
    }

    Ok(())
}
