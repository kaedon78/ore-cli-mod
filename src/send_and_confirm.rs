use std::{
    io::{stdout, Write},
    time::Duration,
};
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    client_error::{ClientError, ClientErrorKind, Result as ClientResult},
    rpc_config::{RpcSendTransactionConfig},
};
use solana_program::instruction::Instruction;
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    signature::{Signature,Signer, Keypair},
    transaction::Transaction,
};

use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding};

const NONCE_RENT: u64 = 1_447_680;

pub struct NonceManager {
    pub rpc_client: std::sync::Arc<RpcClient>,
    pub authority: solana_sdk::pubkey::Pubkey,
    pub capacity: u64,
    pub idx: u64,
}
impl NonceManager {
    pub fn new(rpc_client: std::sync::Arc<RpcClient>, authority: solana_sdk::pubkey::Pubkey, capacity: u64) -> Self {
        NonceManager {
            rpc_client,
            authority,
            capacity,
            idx: 0,
        }
    }

    pub async fn try_init_all(&mut self, payer: &solana_sdk::signer::keypair::Keypair) -> Vec<Result<Signature, solana_client::client_error::ClientError>> {
        let (blockhash, _) = self.rpc_client
            .get_latest_blockhash_with_commitment(CommitmentConfig::finalized()).await
            .unwrap_or_default();
        let mut sigs = vec![];
        for _ in 0..self.capacity {
            let nonce_account = self.next();
            let ixs = self.maybe_create_ixs(&nonce_account.pubkey()).await;
            if ixs.is_none() {
                continue;
            }
            let ixs = ixs.unwrap();
            let tx = Transaction::new_signed_with_payer(&ixs, Some(&payer.pubkey()), &[&payer, &nonce_account], blockhash);
            sigs.push(self.rpc_client.send_transaction(&tx).await);
        }
        sigs
    }

    fn next_seed(&mut self) -> u64 {
        let ret = self.idx;
        self.idx = (self.idx + 1) % self.capacity;
        ret
    }

    pub fn next(&mut self) -> solana_sdk::signer::keypair::Keypair {
        let seed = format!("Nonce:{}:{}", self.authority.clone(), self.next_seed());
        let seed = sha256::digest(seed.as_bytes());
        let kp = solana_sdk::signer::keypair::keypair_from_seed(&seed.as_ref()).unwrap();
        kp
    }

    pub async fn maybe_create_ixs(&mut self, nonce: &solana_sdk::pubkey::Pubkey) -> Option<Vec<Instruction>> {
        if solana_client::nonce_utils::nonblocking::get_account(&self.rpc_client, nonce).await.is_ok() {
            None
        } else {
            Some(solana_sdk::system_instruction::create_nonce_account(
                    &self.authority,
                    &nonce,
                    &self.authority,
                    NONCE_RENT,
            ))
        }
    }
}

use crate::Miner;

const RPC_RETRIES: usize = 0;
const GATEWAY_RETRIES: usize = 75;
const CONFIRM_RETRIES: usize = 1;

const CONFIRM_DELAY: u64 = 0;
const GATEWAY_DELAY: u64 = 600;

impl Miner {
    pub async fn send_and_confirm(
        &self,
        ixs: &[Instruction],
        _dynamic_cus: bool,
        skip_confirm: bool,
        signers: Vec<&Keypair>,
    ) -> ClientResult<Signature> {
        let mut stdout = stdout();
        let signer = signers[0];
        let client = self.rpc_client.clone();
        
        let mut nonce_manager = NonceManager::new(self.rpc_client.clone(), signer.pubkey(), 1 as u64);
        nonce_manager.try_init_all(&signer).await; 

        // Return error if balance is zero
        let balance = client.get_balance(&signer.pubkey()).await.unwrap();
        if balance <= 0 {
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom("Insufficient SOL balance".into()),
            });
        }

        // Build tx
        /*
        let (_hash, slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();
        */
        let send_cfg = RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: Some(CommitmentLevel::Confirmed),
            encoding: Some(UiTransactionEncoding::Base64),
            max_retries: Some(RPC_RETRIES),
            min_context_slot: None,
        };
        
        let msg = solana_sdk::message::Message::new_with_nonce( 
            ixs.to_vec(),
            Some(&signer.pubkey()),
            &nonce_manager.next().pubkey(),
            &signer.pubkey()
        );
        let mut tx = Transaction::new_unsigned(msg.clone());
        
        //let mut tx = Transaction::new_with_payer(ixs, Some(&signer.pubkey()));

        // Update hash before sending transactions
        let (hash, _slot) = client
            .get_latest_blockhash_with_commitment(self.rpc_client.commitment())
            .await
            .unwrap();

        // Submit tx
        // add all the signers
        tx.sign(&signers, hash);

        //let mut sigs = vec![];
        let miningchars = ["\u{1FAA8}","\u{26CF} ","\u{1F48E}"];
        let mut attempts = 0;
        loop {

            match client.send_transaction_with_config(&tx, send_cfg).await {
                Ok(sig) => {
                    //sigs.push(sig);
                    print!("{}", miningchars[attempts%3]);

                    // Confirm tx
                    if skip_confirm {
                        return Ok(sig);
                    }
                    std::thread::sleep(Duration::from_millis(300));
                    for _ in 0..CONFIRM_RETRIES {
                        match client.get_signature_statuses(&[sig]).await {
                            Ok(signature_statuses) => {
                                //println!("Confirms: {:?}", signature_statuses.value);
                                for signature_status in signature_statuses.value {
                                    if let Some(signature_status) = signature_status.as_ref() {
                                        if signature_status.confirmation_status.is_some() {
                                            let current_commitment = signature_status
                                                .confirmation_status
                                                .as_ref()
                                                .unwrap();
                                            match current_commitment {
                                                TransactionConfirmationStatus::Processed => {}
                                                TransactionConfirmationStatus::Confirmed
                                                | TransactionConfirmationStatus::Finalized => {
                                                    println!("{} Success: Transaction landed! sig: {}", chrono::offset::Local::now(), sig);
                                                    return Ok(sig);
                                                }
                                            }
                                        } else {
                                            println!("No status");
                                        }
                                    }
                                }
                            }
                            // Handle confirmation errors
                            Err(err) => {
                                println!("\nError: {:?}", err);
                            }
                        }
                        stdout.flush().ok();
                        std::thread::sleep(Duration::from_millis(CONFIRM_DELAY));
                    }
                    //println!("Transaction did not land");
                }

                // Handle submit errors
                Err(err) => {
                    println!("\nError {:?}", err);
                }
            }
            stdout.flush().ok();

            // Retry
            stdout.flush().ok();
            std::thread::sleep(Duration::from_millis(GATEWAY_DELAY));
            attempts += 1;
            if attempts > GATEWAY_RETRIES {
                return Err(ClientError {
                    request: None,
                    kind: ClientErrorKind::Custom("Max retries".into()),
                });
            }
        }
    }
}