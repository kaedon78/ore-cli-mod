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
        signers: &Vec<&Keypair>,
    ) ->  Transaction {
        let payer = self.payer();    
        let client = self.rpc_client.clone();

        let mut nonce_manager = NonceManager::new(self.rpc_client.clone(), payer.pubkey(), 1 as u64);
        nonce_manager.try_init_all(&payer).await; 

        let msg = solana_sdk::message::Message::new_with_nonce( 
            instructions,
            Some(&payer.pubkey()),
            &nonce_manager.next().pubkey(),
            &payer.pubkey()
        );
        let mut tx = Transaction::new_unsigned(msg.clone());

        self.stats.borrow_mut().add_api_call("getlatestblockhash");
        let (hash, _slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();

        tx.sign(signers, hash);

        tx
    }

    pub async fn create_transaction(
        &self, 
        instructions: Vec<Instruction>,
        signers: &Vec<&Keypair>,
    ) ->  Transaction {
        let payer = self.payer();    
        let client = self.rpc_client.clone();

        self.stats.borrow_mut().add_api_call("getlatestblockhash");
        let (hash, _slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();

        let mut tx = Transaction::new_with_payer(&instructions.into_boxed_slice(), Some(&payer.pubkey()));
        tx.sign(signers, hash);

        tx
    }    
}