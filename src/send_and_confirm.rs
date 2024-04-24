use std::{
    io::{stdout, Write},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use solana_client::{
    //nonblocking::rpc_client::RpcClient,
    client_error::{ClientError, ClientErrorKind, Result as ClientResult},
    rpc_config::RpcSendTransactionConfig,
};
use solana_program::instruction::Instruction;
use solana_sdk::{
    commitment_config::{/*CommitmentConfig, */CommitmentLevel},
    signature::{Signature,Signer, Keypair},
    //transaction::Transaction,
};
use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding};

use crate::{
    constants::{RPC_RETRIES, GATEWAY_RETRIES, CONFIRM_RETRIES, CONFIRM_DELAY, GATEWAY_DELAY},
    Miner
};

impl Miner {
    pub async fn send_and_confirm(
        &self,
        ixs: &[Instruction],
        _dynamic_cus: bool,
        skip_confirm: bool,
        signers: Vec<&Keypair>,
        epoch_threshold: u64,
    ) -> ClientResult<Signature> {
        let mut stdout = stdout();
        let client = self.rpc_client.clone();
        
        // Return error if balance is zero
        /*
        self.stats.borrow_mut().add_api_call("getbalance");
        let balance = client.get_balance(&signer.pubkey()).await.unwrap();
        if balance <= 0 {
            return Err(ClientError {
                request: None,
                kind: ClientErrorKind::Custom("Insufficient SOL balance".into()),
            });
        }*/

        let send_cfg = RpcSendTransactionConfig {
            skip_preflight: false,
            preflight_commitment: Some(CommitmentLevel::Confirmed),
            encoding: Some(UiTransactionEncoding::Base64),
            max_retries: Some(RPC_RETRIES),
            min_context_slot: None,
        };

        let tx = self.create_transaction(
            ixs.to_vec(),
            &signers
        ).await;

        // Submit tx
        let miningchars = ["\u{1FAA8} ","\u{26CF}  ","\u{1F48E} "];
        let mut attempts = 0;
        loop {
                
            if epoch_threshold > 0 {
                let d = UNIX_EPOCH + Duration::from_secs(epoch_threshold-5);
                let n = SystemTime::now();
                if n > d {
                    println!("\nNeed to wait for next epoch...");
                    return Err(ClientError {
                        request: None,
                        kind: ClientErrorKind::Custom("Epoch reset".into()),
                    });
                }
            }
            
            self.stats.borrow_mut().add_api_call("sendtransaction");
            match client.send_transaction_with_config(&tx, send_cfg).await {
                Ok(sig) => {
                    print!("{}", miningchars[attempts%3]);

                    // Confirm tx
                    if skip_confirm {
                        return Ok(sig);
                    }
                    for _ in 0..CONFIRM_RETRIES {
                        self.stats.borrow_mut().add_api_call("getsignaturestatuses");
                        match client.get_signature_statuses(&[sig]).await {
                            Ok(signature_statuses) => {
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
                                                    println!("{} Success: {}\n", chrono::offset::Local::now(), sig);
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
                                if  !err.to_string().contains("0x1") &&
                                    !err.to_string().contains("simulation failed") &&
                                    !err.to_string().contains("This transaction has already been processed"){
                                    println!("\nGet sigs error: {:?}", err.to_string());
                                }
                                return Err(ClientError {
                                    request: None,
                                    kind: ClientErrorKind::Custom(err.to_string().into()),
                                });
                            }
                        }
                        stdout.flush().ok();
                        std::thread::sleep(Duration::from_millis(CONFIRM_DELAY));
                    }
                }
                // Handle submit errors
                Err(err) => {
                    if  !err.to_string().contains("0x1") &&
                        !err.to_string().contains("simulation failed") &&
                        !err.to_string().contains("This transaction has already been processed")
                    {
                        println!("\nSend txn error {:?}", err.to_string());
                    }
                    return Err(ClientError {
                        request: None,
                        kind: ClientErrorKind::Custom(err.to_string().into()),
                    });
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