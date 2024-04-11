use std::str::FromStr;
use ore::{self, state::Proof, utils::AccountDeserialize};
use solana_program::pubkey::Pubkey;
use solana_sdk::{
    instruction::Instruction,
    compute_budget::ComputeBudgetInstruction,
    signature::Signer,
    signer::keypair::Keypair
};
use crate::{cu_limits::CU_LIMIT_CLAIM, utils::proof_pubkey, utils::get_proof, Miner};

impl Miner {
    pub async fn claim(&self, beneficiary: Option<String>, amount: Option<f64>) {
        
        let signer1 = self.signer_by_number(1);
        let client = self.rpc_client.clone();
        let beneficiary = match beneficiary {
            Some(beneficiary) => {
                Pubkey::from_str(&beneficiary).expect("Failed to parse beneficiary address")
            }
            None => self.initialize_ata(&signer1).await,
        };

        let cu_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_CLAIM);
        let cu_price_ix = ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
        let mut claim_ixs: Vec<Instruction> = Vec::new();
        claim_ixs.push(cu_limit_ix);
        claim_ixs.push(cu_price_ix);
        
        let mut has_ore_rewards = false;
        let mut total_rewards_amount = 0;
        let mut signerws = vec![1];

        for w in 1 .. 6 {
            let signer = self.signer_by_number(w);
            let pubkey = signer.pubkey();    
            let proof = get_proof(&self.rpc_client, pubkey).await;
            let rewardtotal = (proof.claimable_rewards as f64) / 10f64.powf(ore::TOKEN_DECIMALS as f64);
            if rewardtotal == 0.0 {
                println!("Nothing to claim for address {}", pubkey);
            }
            else {
                has_ore_rewards = true;
                println!("{} ORE to claim on address {}", rewardtotal, pubkey);
                let amount = if let Some(amount) = amount {
                    (amount * 10f64.powf(ore::TOKEN_DECIMALS as f64)) as u64
                } else {
                    match client.get_account(&proof_pubkey(pubkey)).await {
                        Ok(proof_account) => {
                            let proof = Proof::try_from_bytes(&proof_account.data).unwrap();
                            proof.claimable_rewards
                        }
                        Err(err) => {
                            println!("Error looking up claimable rewards: {:?}", err);
                            return;
                        }
                    }
                };
                total_rewards_amount += amount;
                let ix = ore::instruction::claim(pubkey, beneficiary, amount);
                claim_ixs.push(ix);
                if w > 1 {
                    signerws.push(w);
                }
            }
        }

        let amountf = (total_rewards_amount as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));

        if has_ore_rewards {
            println!("Submitting claim transaction...");
            let signer2 = self.signer_by_number(2);
            let signer3 = self.signer_by_number(3);
            let signer4 = self.signer_by_number(4);
            let signer5 = self.signer_by_number(5);            
            
            let mut signers = vec![&signer1];
            for w in 0..signerws.len() {
                if signerws[w] == 2 { signers.push(&signer2); }
                if signerws[w] == 3 { signers.push(&signer3); }
                if signerws[w] == 4 { signers.push(&signer4); }
                if signerws[w] == 5 { signers.push(&signer5); }
            }

            match self
                .send_and_confirm(&claim_ixs.into_boxed_slice(), false, false, signers)
                .await
            {
                Ok(sig) => {
                    println!("{} Ore Claimed Successfully! to {} : {}", amountf, beneficiary, sig);
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                }
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
            &ore::MINT_ADDRESS,
        );

        // Check if ata already exists
        if let Ok(Some(_ata)) = client.get_token_account(&token_account_pubkey).await {
            return token_account_pubkey;
        }

        // Sign and send transaction.
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            &pubkey,
            &pubkey,
            &ore::MINT_ADDRESS,
            &spl_token::id(),
        );
        println!("Creating token account {}...", token_account_pubkey);
        match self.send_and_confirm(&[ix], true, false, vec![&signer]).await {
            Ok(_sig) => println!("Created token account {:?}", token_account_pubkey),
            Err(e) => println!("Transaction failed: {:?}", e),
        }

        // Return token account address
        token_account_pubkey
    }
}
