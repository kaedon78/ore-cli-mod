#[cfg(feature = "ore")]
use ore::{self, MINT_ADDRESS, TOKEN_DECIMALS, instruction, state::Proof, utils::AccountDeserialize};
#[cfg(feature = "orz")]
use orz::{self, MINT_ADDRESS, TOKEN_DECIMALS, instruction, state::Proof, utils::AccountDeserialize};
#[cfg(feature = "mars")]
use mars::{self, MINT_ADDRESS, TOKEN_DECIMALS, instruction, state::Proof, utils::AccountDeserialize};

use solana_program::pubkey::Pubkey;
use solana_sdk::{
    instruction::Instruction,
    compute_budget::ComputeBudgetInstruction,
    signature::Signer,
    signer::keypair::Keypair
};
use crate::{constants::{CU_LIMIT_CLAIM, TOKEN_NAME}, utils::proof_pubkey, Miner};

impl Miner {
    pub async fn claim_all(&self, amount: Option<f64>) {
        let client = self.rpc_client.clone();
        let mut signerwallets = vec![];
        
        let beneficiary: Pubkey;
        if self.keypair_fee.is_some() {
            beneficiary = self.initialize_ata(self.keypair_fee.as_ref().unwrap()).await;
            signerwallets.push(self.keypair_fee.as_ref().unwrap());
        }
        else {
            beneficiary = self.initialize_ata(&self.wallets[0]).await;
            signerwallets.push(&self.wallets[0]);
        }        

        let cu_limit_amt = 1_000 + (CU_LIMIT_CLAIM * self.wallets.len() as u32);
        let cu_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(cu_limit_amt);
        let cu_price_ix = ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
        let mut claim_ixs: Vec<Instruction> = Vec::new();
        claim_ixs.push(cu_limit_ix);
        claim_ixs.push(cu_price_ix);
        
        let mut has_ore_rewards = false;
        let mut total_rewards_amount = 0;

        for w in 0..self.wallets.len() {
            let pubkey = self.wallets[w].pubkey();
            self.stats.borrow_mut().add_api_call("getaccountinfo");
            let proof = match client.get_account(&proof_pubkey(pubkey)).await {
                Ok(proof_account) => {
                    let proof = Proof::try_from_bytes(&proof_account.data).unwrap().clone();
                    proof
                }
                Err(err) => {
                    println!("Error looking up claimable rewards: {:?}", err);
                    return;
                }
            };

            let rewardtotal = (proof.claimable_rewards as f64) / 10f64.powf(TOKEN_DECIMALS as f64);
            if rewardtotal == 0.0 {
                println!("Nothing to claim for address {}", pubkey);
            }
            else {
                has_ore_rewards = true;
                println!("{} {} to claim on address {}", rewardtotal, TOKEN_NAME, pubkey);
                let amounttoclaim = if let Some(amount) = amount {
                    //println!("Checking {} against {}", amount, proof.claimable_rewards);
                    if (amount * 10f64.powf(TOKEN_DECIMALS as f64)) <= proof.claimable_rewards as f64 {
                        (amount * 10f64.powf(TOKEN_DECIMALS as f64)) as u64
                    }
                    else {
                        0
                    }
                } else {
                    proof.claimable_rewards
                };

                if amounttoclaim > 0 {
                    //println!("Adding {} from wallet {} to claim txn... ", amounttoclaim, w);
                    total_rewards_amount += amounttoclaim;
                    let ix = instruction::claim(pubkey, beneficiary, amounttoclaim);
                    claim_ixs.push(ix);
                }
                else {
                    //println!("Wallet {} does not meet requirements, clearing claim amount", w);
                    //if one wallet doesn't have the amount requested, don't claim anything yet
                    total_rewards_amount = 0;
                }

                if beneficiary != self.wallets[w].pubkey() {
                    signerwallets.push(&self.wallets[w]);
                }
            }
        }

        let amountf = (total_rewards_amount as f64) / (10f64.powf(TOKEN_DECIMALS as f64));

        if has_ore_rewards && amountf > 0.0 {
            println!("Submitting claim transaction...");
            let mut signers = vec![];
            signers.extend(signerwallets);

            match self
                .send_and_confirm(&claim_ixs.into_boxed_slice(), false, false, signers, 0)
                .await
            {
                Ok(sig) => {
                    println!("\n{} {} Claimed Successfully! to {} : {}", amountf, TOKEN_NAME, beneficiary, sig);
                }
                Err(err) => {
                    if !err.to_string().contains("This transaction has already been processed") {
                        println!("\nError: {:?}", err);
                    }
                    else {
                        println!("\n{} {} Claimed Successfully! to {}", amountf, TOKEN_NAME, beneficiary);
                    }
                }
            }
        }
        else {
            if !has_ore_rewards {
                println!("No rewards to claim yet");
            }
            else if amount > Some(0.0) {
                println!("Rewards not sufficient to claim desired amount of {} per wallet", amount.unwrap());
            }
        }
    }

    async fn initialize_ata(&self, signer: &Keypair) -> Pubkey {
        // Initialize client.
        let client = self.rpc_client.clone();

        let pubkey = signer.pubkey();

        // Build instructions.
        let token_account_pubkey = spl_associated_token_account::get_associated_token_address(
            &pubkey,
            &MINT_ADDRESS,
        );

        // Check if ata already exists
        self.stats.borrow_mut().add_api_call("getaccountinfo");
        if let Ok(Some(_ata)) = client.get_token_account(&token_account_pubkey).await {
            return token_account_pubkey;
        }

        // Sign and send transaction.
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            &pubkey,
            &pubkey,
            &MINT_ADDRESS,
            &spl_token::id(),
        );
        println!("Creating token account {}...", token_account_pubkey);
        match self.send_and_confirm(&[ix], true, false, vec![&signer], 0).await {
            Ok(_sig) => println!("Created token account {:?}", token_account_pubkey),
            Err(e) => println!("Transaction failed: {:?}", e),
        }

        // Return token account address
        token_account_pubkey
    }
}
