#[cfg(feature = "ore")]
use ore::{self, ID, state::{Proof, Treasury}, utils::AccountDeserialize, MINT_ADDRESS, PROOF, TREASURY_ADDRESS};
#[cfg(feature = "orz")]
use orz::{self, ID, state::{Proof, Treasury}, utils::AccountDeserialize, MINT_ADDRESS, PROOF, TREASURY_ADDRESS};

use cached::proc_macro::cached;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_program::{pubkey::Pubkey, sysvar};
use solana_sdk::clock::Clock;
use spl_associated_token_account::get_associated_token_address;

pub async fn get_treasury(client: &RpcClient) -> Treasury {
    //println!("Calling getaccount for treasury");
    let data = client
        .get_account_data(&TREASURY_ADDRESS)
        .await
        .expect("Failed to get treasury account");
    *Treasury::try_from_bytes(&data).expect("Failed to parse treasury account")
}

pub async fn get_proof(client: &RpcClient, authority: Pubkey) -> Proof {
    let proof_address = proof_pubkey(authority);
    //println!("Calling getaccount for proof");
    let data = client
        .get_account_data(&proof_address)
        .await
        .expect("Failed to get miner account");
    *Proof::try_from_bytes(&data).expect("Failed to parse miner account")
}

pub async fn get_clock_account(client: &RpcClient) -> Clock {
    //println!("Calling getaccount for clock");
    let data = client
        .get_account_data(&sysvar::clock::ID)
        .await
        .expect("Failed to get miner account");
    bincode::deserialize::<Clock>(&data).expect("Failed to deserialize clock")
}

#[cached]
pub fn proof_pubkey(authority: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[PROOF, authority.as_ref()], &ID).0
}

#[cached]
pub fn treasury_tokens_pubkey() -> Pubkey {
    get_associated_token_address(&TREASURY_ADDRESS, &MINT_ADDRESS)
}
