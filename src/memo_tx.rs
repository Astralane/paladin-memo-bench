use rand::Rng;
use rand::distributions::Alphanumeric;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

pub const MEMO_PROGRAM_ID: pubkey::Pubkey = pubkey!("MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr");
pub fn rand_memo_tx(
    sender: &Keypair,
    cu_price: u64,
    block_hash: solana_sdk::hash::Hash,
    prefix: &str,
) -> Transaction {
    let mut ixs = vec![];
    ixs.push(ComputeBudgetInstruction::set_compute_unit_price(cu_price));
    ixs.push(ComputeBudgetInstruction::set_compute_unit_limit(25_000));
    ixs.push(Instruction::new_with_borsh(
        MEMO_PROGRAM_ID,
        &generate_random_string(prefix, 8),
        vec![AccountMeta::new(sender.pubkey(), true)],
    ));
    Transaction::new_signed_with_payer(&ixs, Some(&sender.pubkey()), &[&sender], block_hash)
}

fn generate_random_string(prefix: &str, length: usize) -> String {
    let random_part: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect();

    format!("{}{}", prefix, random_part)
}
