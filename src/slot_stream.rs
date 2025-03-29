use futures_util::StreamExt;
use futures_util::stream::BoxStream;
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_rpc_client_api::response::SlotUpdate;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub async fn create_palidator_slot_stream<'a>(
    pubsub_client: &'a PubsubClient,
    schedule: Arc<HashSet<u64>>,
) -> anyhow::Result<BoxStream<'a, u64>> {
    let (mut stream, _unsub) = pubsub_client.slot_updates_subscribe().await?;
    let last_slot = Arc::new(AtomicU64::new(0));
    Ok(stream
        .filter_map(move |update| {
            let schedule_cl = Arc::clone(&schedule);
            let last_slot_cl: Arc<AtomicU64> = Arc::clone(&last_slot);
            async move {
                let slot = match update {
                    SlotUpdate::FirstShredReceived { slot, .. } => Some(slot),
                    SlotUpdate::Completed { slot, .. } => Some(slot + 1),
                    _ => None,
                };
                match slot {
                    Some(slot) => {
                        if slot > last_slot_cl.load(Ordering::Relaxed) {
                            last_slot_cl.store(slot, Ordering::Relaxed);
                            if schedule_cl.contains(&slot) {
                                return Some(slot);
                            }
                        }
                        return None;
                    }
                    None => None,
                }
            }
        })
        .boxed())
}
