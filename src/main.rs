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
use tracing::{info, warn};

mod memo_tx;
mod slot_stream;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    ws_rpc: String,
    http_rpc: String,
    keypair_path: String,
    paladin_rpc: String,
    duration_mins: u64,
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
        .take_until(Box::pin(tokio::time::sleep(Duration::from_secs(
            config.duration_mins * 60,
        ))));

    let mut handles = Vec::new();

    info!("memo bench started ...");
    let cu_price = config.cu_price_micro_lamports;
    while let Some(slot) = stream.next().await {
        let signer = signer.clone();
        let sender = sender.clone();
        let _rpc = rpc.clone();
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
    let mut statuses = Vec::new();

    // call the signatures in chunks of 10
    for sig_batch in signatures.chunks(10){
        let mut status_batch = rpc.get_signature_statuses(&sig_batch).await?.value;
        statuses.append(&mut status_batch);
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let mut slot_latencies = Vec::new();
    let mut not_landed_slots = Vec::new();
    let mut total_landed = 0;
    for (status, signature) in statuses.iter().zip(&signatures) {
        let send_slot = *signatures_map.get(signature).unwrap();
        if let Some(landed_slot) = status.as_ref().map(|s| s.slot) {
            total_landed += 1;
            slot_latencies.push(landed_slot - send_slot);
            continue;
        }
        not_landed_slots.push(send_slot);
    }

    info!(
        "total landed / total sent: {}/{}",
        total_landed,
        signatures.len()
    );

    if !slot_latencies.is_empty() {
        let average_latency =
            slot_latencies.iter().sum::<u64>() as f64 / slot_latencies.len() as f64;
        info!("avg latencies: {:?}", average_latency);
    } else {
        warn!("no transaction landed!")
    }

    info!("not landed slots: {:?}", not_landed_slots);

    let not_landed_validators = not_landed_slots
        .iter()
        .map(|slot| reverse_map.get(slot).unwrap().clone())
        .collect::<Vec<_>>();

    info!("not landed validators {:?}", not_landed_validators);
    Ok(())
}
