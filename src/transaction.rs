#[cfg(feature = "ore")]
use ore::{BUS_ADDRESSES, BUS_COUNT, TREASURY_ADDRESS};
#[cfg(feature = "orz")]
use orz::{BUS_ADDRESSES, BUS_COUNT, TREASURY_ADDRESS};
#[cfg(feature = "mars")]
use mars::{BUS_ADDRESSES, BUS_COUNT, TREASURY_ADDRESS};

use std::{
    io::{stdout, Write},
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use solana_program::{
        instruction::Instruction, 
        //message::Message, pubkey, sysvar
};
use solana_address_lookup_table_program::{self, state::AddressLookupTable};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    pubkey::Pubkey,
    signature::{Signer, Keypair},
    transaction::{Transaction, VersionedTransaction},
    address_lookup_table::AddressLookupTableAccount,
    signature::Signature,
    message::{v0, VersionedMessage},
};
use solana_client::{
    //nonblocking::rpc_client::RpcClient,
    client_error::{ClientError, ClientErrorKind, Result as ClientResult},
    rpc_config::RpcSendTransactionConfig,
};
use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding};
use bincode::serialize;

use crate::{
    constants::{CONFIRM_DELAY, CONFIRM_RETRIES, GATEWAY_RETRIES, RPC_RETRIES},
    Miner, 
    //nonce_manager::NonceManager
};

impl Miner {
    /*
    pub async fn _create_nonce_transaction(
        &self, 
        instructions: Vec<Instruction>,
        signers: &Vec<&Keypair>,
    ) ->  Transaction {
        let payer = self.payer();    
        let client = self.rpc_client.clone();

        let mut nonce_manager = NonceManager::new(self.rpc_client.clone(), payer.pubkey(), 1 as u64);
        nonce_manager.try_init_all(&payer).await; 

        let msg = Message::new_with_nonce( 
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
    */

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

        let gateway_delay_adj:u64;
        {
            gateway_delay_adj = self.stats.borrow_mut().get_adj_gateway_delay();
        }

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
                let d = UNIX_EPOCH + Duration::from_secs(epoch_threshold-3); //match 3 sec offset with delay below
                let n = SystemTime::now();
                if n > d {
                    println!("\nNeed to wait for next epoch...");
                    std::thread::sleep(Duration::from_millis(3100));
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
            std::thread::sleep(Duration::from_millis(gateway_delay_adj));
            attempts += 1;
            if attempts > GATEWAY_RETRIES {
                return Err(ClientError {
                    request: None,
                    kind: ClientErrorKind::Custom("Max retries".into()),
                });
            }
        }
    }

    pub async fn _create_lookup_table(&self) {
        let payer = self.payer();
        let client = self.rpc_client.clone();

        println!("Creating lookup table account");
        let recent_slot = client
            .get_slot_with_commitment(CommitmentConfig::finalized())
            .await
            .unwrap();
        let (create_ix, table_pk) =
            solana_address_lookup_table_program::instruction::create_lookup_table(
                payer.pubkey(),
                payer.pubkey(),
                recent_slot,
            );

        let latest_blockhash = client.get_latest_blockhash().await.unwrap();
        client
            .send_and_confirm_transaction(&Transaction::new_signed_with_payer(
                &[create_ix],
                Some(&payer.pubkey()),
                &[&payer],
                latest_blockhash,
            ))
            .await
            .unwrap();

        println!("Loop to extend the address lookup table");
        let mut signature = Signature::default();
        let latest_blockhash = client.get_latest_blockhash().await.unwrap();
        
        for buses in BUS_ADDRESSES.iter().map(|b|*b).collect::<Vec<Pubkey>>().chunks(BUS_COUNT) {
            let mut new_addresses = buses.to_vec();
            new_addresses.extend(vec![TREASURY_ADDRESS]);

            let extend_ix = solana_address_lookup_table_program::instruction::extend_lookup_table(
                table_pk,
                payer.pubkey(),
                Some(payer.pubkey()),
                new_addresses,
            );

            signature = client
                .send_and_confirm_transaction(&Transaction::new_signed_with_payer(
                    &[extend_ix],
                    Some(&payer.pubkey()),
                    &[&payer],
                    latest_blockhash,
                ))
                .await
                .unwrap();
        }
        
        client
            .confirm_transaction_with_spinner(
                &signature,
                &latest_blockhash,
                CommitmentConfig::finalized(),
            )
            .await
            .unwrap();
        
    }

    pub async fn _update_lookup_table(&self) {
        let payer = self.payer();
        let client = self.rpc_client.clone();
        let table_pk = Pubkey::from_str("DaAfJ9prCxsdRFVGcqxJZ72Z5jAQTnLWnCnS2FBCR3hv").unwrap();

        println!("Loop to extend the address lookup table");
        let latest_blockhash = client.get_latest_blockhash().await.unwrap();
        
        let mut new_addresses = vec![];
        /*new_addresses.push(Pubkey::find_program_address(&[PROOF, self.wallets[0].pubkey().as_ref()], &id()).0);
        new_addresses.push(Pubkey::find_program_address(&[PROOF, self.wallets[1].pubkey().as_ref()], &id()).0);
        new_addresses.push(Pubkey::find_program_address(&[PROOF, self.wallets[2].pubkey().as_ref()], &id()).0);
        new_addresses.push(Pubkey::find_program_address(&[PROOF, self.wallets[3].pubkey().as_ref()], &id()).0);
        new_addresses.push(Pubkey::find_program_address(&[PROOF, self.wallets[4].pubkey().as_ref()], &id()).0);
        new_addresses.push(sysvar::slot_hashes::id());
        */
        new_addresses.push(self.wallets[0].pubkey());
        new_addresses.push(self.wallets[1].pubkey());
        new_addresses.push(self.wallets[2].pubkey());
        new_addresses.push(self.wallets[3].pubkey());
        new_addresses.push(self.wallets[4].pubkey());

        let extend_ix = solana_address_lookup_table_program::instruction::extend_lookup_table(
            table_pk,
            payer.pubkey(),
            Some(payer.pubkey()),
            new_addresses,
        );

        let signature = client
            .send_and_confirm_transaction(&Transaction::new_signed_with_payer(
                &[extend_ix],
                Some(&payer.pubkey()),
                &[&payer],
                latest_blockhash,
            ))
            .await
            .unwrap();
                
        client
            .confirm_transaction_with_spinner(
                &signature,
                &latest_blockhash,
                CommitmentConfig::finalized(),
            )
            .await
            .unwrap();
        
    }

    pub async fn _create_lookup_table_tx(&self, instructions: Vec<Instruction>) -> VersionedTransaction {
        let payer = self.payer();
        let client = self.rpc_client.clone();
        //let table_pk = Pubkey::from_str("EBjpJpfFEjpRNknPhmeEJv6u3nQXajJSym8vL83Fyyyu").unwrap();
        let table_pk = Pubkey::from_str("DaAfJ9prCxsdRFVGcqxJZ72Z5jAQTnLWnCnS2FBCR3hv").unwrap();

        let latest_blockhash = client.get_latest_blockhash().await.unwrap();

        let mut signers: Vec<&Keypair> = self.wallets.iter().collect();
        if self.payer() != &self.wallets[0] {
            signers.insert(0, self.payer());
        }

        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&payer.pubkey()),
            &signers,
            latest_blockhash,
        );
        let serialized_tx = serialize(&tx).unwrap();

        println!("This legacy serialized tx is {} bytes", serialized_tx.len());

        println!("Wait some arbitrary amount of time to please the address lookup table");
        std::thread::sleep(Duration::from_millis(5000));

        let versioned_tx = self._create_tx_with_address_table_lookup(instructions, table_pk, &payer, &signers).await.unwrap();
        let serialized_versioned_tx = serialize(&versioned_tx).unwrap();
        println!(
            "The serialized versioned tx is {} bytes",
            serialized_versioned_tx.len()
        );

        versioned_tx
    }

    pub async fn _create_tx_with_address_table_lookup(
        &self,
        instructions: Vec<Instruction>,
        address_lookup_table_key: Pubkey,
        payer: &Keypair,
        signers: &Vec<&Keypair>,
    ) -> ClientResult<VersionedTransaction> {
        let client = self.rpc_client.clone();
        
        let raw_account = client.get_account_data(&address_lookup_table_key).await.unwrap();
        match AddressLookupTable::deserialize(&raw_account) {
            Ok(address_lookup_table) => {
                let address_lookup_table_account = AddressLookupTableAccount {
                    key: address_lookup_table_key,
                    addresses: address_lookup_table.addresses.to_vec(),
                };

                println!("addresses: {:?}", address_lookup_table.addresses.to_vec());

                let latest_blockhash = client.get_latest_blockhash().await.unwrap();
                
                let v0_message = VersionedMessage::V0(v0::Message::try_compile(
                    &payer.pubkey(),
                    &instructions.into_boxed_slice(),
                    &[address_lookup_table_account],
                    latest_blockhash,
                ).map_err(|err: solana_sdk::message::CompileError| ClientError {
                    request: None, 
                    kind: ClientErrorKind::Custom(err.to_string()) 
                })?);

                for ixn in v0_message.instructions() {
                    println!("instruction has accounts: {:?}", ixn.accounts);
                }
                for atlw in &v0_message.address_table_lookups().unwrap()[0].writable_indexes {
                    println!("address_table_lookups writable: {:?}", atlw);
                }
                for atlr in &v0_message.address_table_lookups().unwrap()[0].readonly_indexes {
                    println!("address_table_lookups readable: {:?}", atlr);
                }

                assert!(v0_message.address_table_lookups().unwrap().len() > 0);
                
                let tx = VersionedTransaction::try_new(
                    v0_message,
                    signers,
                )?;
            
                Ok(tx)
            }
            Err(_) => {
                return Err(ClientError {
                    request: None,
                    kind: ClientErrorKind::Custom("create_tx_with_address_table_lookup failed".into()),
                });
            }
        }
    }
}