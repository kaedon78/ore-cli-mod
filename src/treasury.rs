use crate::{
    cu_limits::{CU_LIMIT_RESET},
    utils::{get_treasury, treasury_tokens_pubkey},
    Miner,
};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    signature::Signer,
    clock::Clock
};
use ore::{self, state::Treasury, EPOCH_DURATION};
use rand::Rng;
/*
use chrono::{Local};
use chrono::prelude::DateTime;
use std::{
    time::{ Duration, UNIX_EPOCH},
};
*/
const RESET_ODDS: u64 = 20;

impl Miner {
    pub async fn treasury(&self) {
        let client = self.rpc_client.clone();
        //println!("Calling getaccount for treasury");
        if let Ok(Some(treasury_tokens)) = client.get_token_account(&treasury_tokens_pubkey()).await
        {
            let treasury = get_treasury(&self.rpc_client).await;
            let balance = treasury_tokens.token_amount.ui_amount_string;
            println!("{:} ORE", balance);
            println!("Admin: {}", treasury.admin);
            println!("Difficulty: {}", treasury.difficulty.to_string());
            println!("Last reset at: {}", treasury.last_reset_at);
            println!(
                "Reward rate: {} ORE",
                (treasury.reward_rate as f64) / 10f64.powf(ore::TOKEN_DECIMALS as f64)
            );
            println!(
                "Total claimed rewards: {} ORE",
                (treasury.total_claimed_rewards as f64) / 10f64.powf(ore::TOKEN_DECIMALS as f64)
            );
        }
    }

    pub async fn check_epoch_reset(&self, treasury:&Treasury, clock:&Clock) -> bool {
        // Reset epoch, if needed
        /*
        let d = UNIX_EPOCH + Duration::from_secs(treasury.last_reset_at.try_into().unwrap());
        let datetime = DateTime::<Local>::from(d);
        let timestamp_str = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
        println!("Treasury last reset at: {}", timestamp_str);
        */    
        let threshold = treasury.last_reset_at.saturating_add(EPOCH_DURATION);
        /*
        let d2 = UNIX_EPOCH + Duration::from_secs(threshold.try_into().unwrap());
        let datetime2 = DateTime::<Local>::from(d2);
        let timestamp_str2 = datetime2.format("%Y-%m-%d %H:%M:%S").to_string();
        println!("Treasury next reset at: {}", timestamp_str2);
        */

        if clock.unix_timestamp.ge(&threshold) {
            let mut rng = rand::thread_rng();    
            // There are a lot of miners right now, so randomly select into submitting tx
            //println!("\nChecking reset odds...");
            if rng.gen_range(0..RESET_ODDS).eq(&0) {
                //println!("\nSending epoch reset transaction...");
                let cu_limit_ix =
                    ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_RESET);
                let cu_price_ix =
                    ComputeBudgetInstruction::set_compute_unit_price(1);
                let signer = self.signer_by_number(1);    
                let reset_ix = ore::instruction::reset(signer.pubkey());
                self.send_and_confirm(&[cu_limit_ix, cu_price_ix, reset_ix], false, true, vec![&signer], 0)
                    .await
                    .ok();
                return true
            }
            else {
                return false
            }
        }
        else {
            return true
        }
    }
}
