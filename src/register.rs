#[cfg(feature = "ore")]
use ore::{self, instruction};
#[cfg(feature = "orz")]
use orz::{self, instruction};
#[cfg(feature = "mars")]
use mars::{self, instruction};

use solana_sdk::{
    signature::Signer,
    signer::keypair::Keypair,
    transaction::Transaction,
};
use std::time::Duration;
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

        if self.payer() != &self.wallets[0] {
            //inner instruction needs to be paid by wallet[x]
            self.stats.borrow_mut().add_api_call("getbalance");
            let current_balance = client.get_balance(&signer.pubkey()).await.unwrap();
            if current_balance < 3_000_000 {
                self.cover_account_cost(signer, self.payer(), current_balance).await;
                println!("Sent sol from payer to cover registration...");
            }            
        }

        //Add registration transaction
        let ix = instruction::register(pubkey);
        let signers = vec![self.payer(), signer];
        // Sign and send transaction.
        match self.send_and_confirm(&[ix], true, false, signers, 0)
            .await {
                Ok(sig) => {
                    println!("{} Registration successful: {}", chrono::offset::Local::now(), sig);
                }
                Err(err) => {
                    if  err.to_string().contains("This transaction has already been processed"){
                        println!("{} Registration successful", chrono::offset::Local::now());
                    }
                    else {
                        println!("{} Txn error: {}", chrono::offset::Local::now(), err.to_string());
                        std::thread::sleep(Duration::from_millis(1000));
                    }
                }
            }
    }

    pub async fn cover_account_cost(&self, signer: &Keypair, payer: &Keypair, current_balance: u64) {
        
        let client = self.rpc_client.clone();
        let transfer_amt = 3_000_000 - current_balance;
        let ix = solana_program::system_instruction::transfer(&self.payer().pubkey(), &signer.pubkey(), transfer_amt);
        let latest_blockhash = client.get_latest_blockhash().await.unwrap();
        let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], latest_blockhash);
        client.send_and_confirm_transaction(&tx).await.unwrap();
    }
}
