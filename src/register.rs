use std::str::FromStr;
use solana_sdk::signature::Signer;
use solana_sdk::pubkey::Pubkey;
use crate::{utils::proof_pubkey, Miner};

impl Miner {
    pub async fn register(&self, address: Option<String>) {
        // Return early if miner is already registered
        let signer = self.signer();
        let client = self.rpc_client.clone();

        let pubkey = if let Some(address) = address {
            if let Ok(address) = Pubkey::from_str(&address) {
                address
            } else {
                println!("Invalid address: {:?}", address);
                signer.pubkey()
            }
        } else {
            signer.pubkey()
        };
        
        let proof_address = proof_pubkey(pubkey);

        if client.get_account(&proof_address).await.is_ok() {
            println!("Registration OK...");    
            return;
        }

        // Sign and send transaction.
        println!("Generating challenge...");
        let ix = ore::instruction::register(pubkey);
        self.send_and_confirm(&[ix], true, false, vec![&signer])
            .await
            .expect("Transaction failed");
    }
}
