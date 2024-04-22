use ore::TREASURY_ADDRESS;
use solana_sdk::signature::Signer;

use crate::Miner;

impl Miner {
    pub async fn initialize(&self) {
        let mut init_txns: Vec<Instruction> = Vec::new();
        println!("Initializing program...");
        let client = RpcClient::new_with_commitment(self.cluster.clone(), CommitmentConfig::confirmed());
        self.stats.borrow_mut().add_api_call("getaccountinfo");
        // Return early if program is initialized
        if client.get_account(&TREASURY_ADDRESS).await.is_ok() {
            return;
        }

        // Sign and send transaction.
        let ix = ore::instruction::initialize(self.signer().pubkey());

        self.send_and_confirm(&[ix].into_boxed_slice(), false, false, 0)
            .await
            .expect("Transaction failed");        
        }
    }
}
