use crate::memo_tx::rand_memo_tx;
use crate::slot_stream::create_palidator_slot_stream;
use figment::Figment;
use figment::providers::Env;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::config::RpcSendTransactionConfig;
use solana_sdk::signature::{EncodableKey, Keypair, Signature};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::info;

mod memo_tx;
mod slot_stream;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    ws_rpc: String,
    http_rpc: String,
    keypair_path: String,
    paladin_rpc: String,
    num_leaders: usize,
    cu_price_micro_lamports: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();
    let config: Config = Figment::new().merge(Env::raw()).extract().unwrap();
    let pub_sub = PubsubClient::new(&config.ws_rpc).await?;
    let rpc = Arc::new(RpcClient::new(config.http_rpc));
    let sender = Arc::new(RpcClient::new(config.paladin_rpc));
    let signer = Arc::new(Keypair::read_from_file(config.keypair_path).unwrap());
    let client = Arc::new(reqwest::Client::new());
    let block_hash = Arc::new(RwLock::new(None));

    let resp = client
        .request(
            reqwest::Method::GET,
            "http://paris.gateway.astralane.io/api/palidators",
        )
        .send()
        .await?;
    let body = resp.text().await?;
    info!("fetched palidator schedule");

    let block_hash_cl = block_hash.clone();
    let rpc_cl = rpc.clone();
    tokio::spawn(async move {
        loop {
            let hash = rpc_cl.get_latest_blockhash().await.unwrap();
            {
                let mut lock = block_hash_cl.write().unwrap();
                *lock = Some(hash);
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });

    while block_hash.read().unwrap().is_none() {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    info!("blockhash initial update complete");

    let schedule_map = serde_json::from_str::<HashMap<String, Vec<u64>>>(&body)?;
    let reverse_map = schedule_map
        .iter()
        .flat_map(|(k, vec)| vec.iter().map(move |&v| (v, k.to_owned())))
        .collect::<HashMap<_, _>>();

    let schedule = schedule_map
        .values()
        .into_iter()
        .flatten()
        .cloned()
        .collect();

    let mut stream = create_palidator_slot_stream(&pub_sub, Arc::new(schedule))
        .await?
        .take(config.num_leaders);

    let mut handles = Vec::new();

    info!("running stream");
    let cu_price = config.cu_price_micro_lamports;
    while let Some(slot) = stream.next().await {
        let signer = signer.clone();
        let sender = sender.clone();
        let block_hash = {
            let lock = block_hash.read().unwrap();
            lock.unwrap()
        };

        let hdl = tokio::spawn(async move {
            let tx = rand_memo_tx(&signer, cu_price, block_hash, "TESTING");
            let sig = sender
                .send_transaction_with_config(
                    &tx,
                    RpcSendTransactionConfig {
                        skip_preflight: true,
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            println!("sig: {:?}", sig);
            (sig, slot)
        });
        handles.push(hdl);
    }

    info!("calculating results...");
    let mut signatures_map = HashMap::new();
    for handle in handles {
        let (sig, slot) = handle.await?;
        signatures_map.insert(sig, slot);
    }

    //wait 10 seconds for confirmation of all txns
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    let signatures = signatures_map.keys().cloned().collect::<Vec<_>>();
    let statuses = rpc.get_signature_statuses(&signatures).await?.value;

    let mut diffs = Vec::new();
    let mut not_landed_slots = Vec::new();
    let mut total_landed = 0;
    for (status, signature) in statuses.iter().zip(&signatures) {
        let send_slot = *signatures_map.get(signature).unwrap();
        if let Some(landed_slot) = status.as_ref().map(|s| s.slot) {
            total_landed += 1;
            diffs.push(landed_slot - send_slot);
            continue;
        }
        not_landed_slots.push(send_slot);
    }

    info!(
        "total landed / total sent: {}/{}",
        total_landed,
        signatures.len()
    );
    info!("slot latencies: {:?}", diffs);

    let not_landed_validators = not_landed_slots
        .iter()
        .map(|slot| reverse_map.get(slot).unwrap().clone())
        .collect::<Vec<_>>();

    info!("not landed valdiators {:?}", not_landed_validators);
    Ok(())
}
