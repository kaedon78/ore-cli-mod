use solana_sdk::{
    signature::Signer,
    signer::keypair::Keypair
};
use crate::{utils::proof_pubkey, Miner};

impl Miner {
    pub async fn register_by_number(&self, keypair_number: u64) {
        self.register(&self.signer_by_number(keypair_number)).await
    }

    pub async fn register(&self, signer: &Keypair) {
        // Return early if miner is already registered
        let client = self.rpc_client.clone();
        let pubkey = signer.pubkey();
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
