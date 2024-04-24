#[cfg(feature = "ore")]
use ore::{self, instruction};
#[cfg(feature = "orz")]
use orz::{self, instruction};

use solana_sdk::{
    signature::Signer,
    signer::keypair::Keypair
};
use crate::{utils::proof_pubkey, Miner};

impl Miner {
    pub async fn register_all(&self) {
        for w in 0..self.wallets.len() {
            println!("Generating challenge for wallet {}...", w);
            self.register(&self.wallets[w]).await
        }
    }

    pub async fn register(&self, signer: &Keypair) {
        // Return early if miner is already registered
        let client = self.rpc_client.clone();
        let pubkey = signer.pubkey();
        let proof_address = proof_pubkey(pubkey);
        self.stats.borrow_mut().add_api_call("getaccountinfo");
        if client.get_account(&proof_address).await.is_ok() {
            println!("Registration OK...");    
            return;
        }

        // Sign and send transaction.
        let ix = instruction::register(pubkey);
        self.send_and_confirm(&[ix], true, false, vec![&signer], 0)
            .await
            .expect("Transaction failed");
    }
}
