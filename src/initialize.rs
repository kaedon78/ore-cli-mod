use ore::TREASURY_ADDRESS;
use solana_sdk::signature::Signer;

use crate::Miner;

impl Miner {
    pub async fn initialize(&self) {
        let mut init_txns: Vec<Instruction> = Vec::new();
        println!("Initializing miners");
        for wallet in 1..6 {
            // Return early if program is initialized
            let client = RpcClient::new_with_commitment(self.cluster.clone(), CommitmentConfig::confirmed());
            if client.get_account(&TREASURY_ADDRESS).await.is_ok() {
                return;
            }

            // Sign and send transaction.
            let ix = ore::instruction::initialize(signer.pubkey());
            init_txns.push(ix);        
        }

        self.send_and_confirm(&init_txns.into_boxed_slice(), false, false)
            .await
            .expect("Transaction failed");        
        }
    }
}
