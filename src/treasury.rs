#[cfg(feature = "ore")]
use ore::{self, instruction, state::Treasury, EPOCH_DURATION, TOKEN_DECIMALS};
#[cfg(feature = "orz")]
use orz::{self, instruction, state::Treasury, EPOCH_DURATION, TOKEN_DECIMALS};

use crate::{
    constants::{CU_LIMIT_RESET, TOKEN_NAME},
    utils::{get_treasury, get_clock_account, treasury_tokens_pubkey},
    Miner,
};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    signature::Signer,
    clock::Clock
};
use std::time::{
    Duration, SystemTime, UNIX_EPOCH
};

use rand::Rng;
use chrono::{DateTime, Local};
use crate::constants::RESET_ODDS;

impl Miner {
    pub async fn treasury(&self) {
        let client = self.rpc_client.clone();
        //println!("Calling getaccount for treasury");
        self.stats.borrow_mut().add_api_call("getaccountinfo");
        if let Ok(Some(treasury_tokens)) = client.get_token_account(&treasury_tokens_pubkey()).await
        {
            self.stats.borrow_mut().add_api_call("getaccountinfo");
            let treasury = get_treasury(&self.rpc_client).await;
            let balance = treasury_tokens.token_amount.ui_amount_string;
            println!("{:} {}", balance, TOKEN_NAME);
            println!("Admin: {}", treasury.admin);
            println!("Difficulty: {}", treasury.difficulty.to_string());
            println!("Last reset at: {}", treasury.last_reset_at);
            println!("Reward rate: {} {}", (treasury.reward_rate as f64) / 10f64.powf(TOKEN_DECIMALS as f64), TOKEN_NAME);
            println!("Total claimed rewards: {} {}", (treasury.total_claimed_rewards as f64) / 10f64.powf(TOKEN_DECIMALS as f64), TOKEN_NAME);
        }
    }

    pub async fn wait_for_next_epoch(&self, treasury:&Treasury, clock:&Clock) {
        
        let mut threshold = treasury.last_reset_at.saturating_add(EPOCH_DURATION);
        let mut epoch_valid = clock.unix_timestamp.lt(&threshold);

        if epoch_valid {
            let d2 = UNIX_EPOCH + Duration::from_secs(threshold.try_into().unwrap());
            let duration_until_d2 = d2.duration_since(SystemTime::now()).unwrap_or(Duration::from_secs(0));
            //wait until epoch ends
            println!("Waiting for Epoch end...");
            std::thread::sleep(duration_until_d2);
            epoch_valid = false;
        }

        loop {
            //wait for epoch to become valid    
            println!("Waiting for Epoch reset...");
            self.stats.borrow_mut().add_api_call("getaccountinfo");
            let treasury = &get_treasury(&self.rpc_client).await;
            self.stats.borrow_mut().add_api_call("getaccountinfo");
            let clock = &get_clock_account(&self.rpc_client).await;
            threshold = treasury.last_reset_at.saturating_add(EPOCH_DURATION);
            epoch_valid = clock.unix_timestamp.lt(&threshold);
            if epoch_valid {
                std::thread::sleep(Duration::from_millis(1000));
                break;
            }
            std::thread::sleep(Duration::from_millis(2000));
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
                println!("\nSending epoch reset transaction...");
                let cu_limit_ix =
                    ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_RESET);
                let cu_price_ix =
                    ComputeBudgetInstruction::set_compute_unit_price(1);
                let reset_ix = instruction::reset(self.payer().pubkey());
                self.send_and_confirm(&[cu_limit_ix, cu_price_ix, reset_ix], false, true, vec![&self.payer()], 0)
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

    pub fn get_last_reset_local(&self, treasury:&Treasury) -> DateTime::<Local> {
        let d = UNIX_EPOCH + Duration::from_secs(treasury.last_reset_at.try_into().unwrap());
        let datetime = DateTime::<Local>::from(d);
        datetime
    }

    pub fn get_next_reset_local(&self, treasury:&Treasury) -> DateTime::<Local> {
        let threshold = treasury.last_reset_at.saturating_add(EPOCH_DURATION);
        let d = UNIX_EPOCH + Duration::from_secs(threshold.try_into().unwrap());
        let datetime = DateTime::<Local>::from(d);
        datetime
    }

    pub fn get_reward_rate(&self, treasury:&Treasury) -> f64 {
        (treasury.reward_rate as f64) / (10f64.powf(TOKEN_DECIMALS as f64))
    }

    pub fn print_treasury_stats(&self, treasury:&Treasury) {
        println!("Treasury difficulty: {}", treasury.difficulty.to_string());
        println!("Treasury last reset at: {}", self.get_last_reset_local(treasury).format("%Y-%m-%d %H:%M:%S").to_string());
        println!("Treasury next reset at: {}", self.get_next_reset_local(treasury).format("%Y-%m-%d %H:%M:%S").to_string());
    }
}
