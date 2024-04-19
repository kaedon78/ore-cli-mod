use solana_program::instruction::Instruction;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Signer, Keypair},
    transaction::Transaction,
};

use crate::{Miner, nonce_manager::NonceManager};

impl Miner {
    pub async fn create_nonce_transaction(
        &self, 
        instructions: Vec<Instruction>,
        payer: Option<&Pubkey>,
        nonce_authority_pubkey: &Pubkey,
        signers: &Vec<&Keypair>,
    ) ->  Transaction {
        let signer = signers[0];    
        let client = self.rpc_client.clone();

        let mut nonce_manager = NonceManager::new(self.rpc_client.clone(), signer.pubkey(), 1 as u64);
        nonce_manager.try_init_all(&signer).await; 

        let msg = solana_sdk::message::Message::new_with_nonce( 
            instructions,
            payer,
            &nonce_manager.next().pubkey(),
            nonce_authority_pubkey
        );
        let mut tx = Transaction::new_unsigned(msg.clone());

        let (hash, _slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();

        tx.sign(signers, hash);

        tx
    }
}