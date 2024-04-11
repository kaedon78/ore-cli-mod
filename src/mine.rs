use fastrand;
use crossbeam::thread;
use std::{
    collections::HashMap,
    io::{stdout, Write},
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::{Instant, Duration},
};
use rand::Rng;
use ore::{self, state::Bus, BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION};
use solana_program::{keccak::HASH_BYTES, program_memory::sol_memcmp, pubkey::Pubkey};
use solana_sdk::{
    instruction::Instruction,
    compute_budget::ComputeBudgetInstruction,
    keccak::{hashv, Hash as KeccakHash},
    signature::Signer,
    signer::keypair::Keypair
};

use crate::{
    cu_limits::{CU_LIMIT_MINE, CU_LIMIT_RESET},
    utils::{get_clock_account, get_proof, get_treasury},
    Miner,
};

// Odds of being selected to submit a reset tx
const RESET_ODDS: u64 = 20;

/*
struct SharedNextHashRangeData {
    min_hamming_distance: usize,
    last_hamming_distance: usize,
    range_step: usize,
    range_decreases: usize,
    nonce_min: usize,
    nonce_max: usize,
}        
*/

const WALLETS: u64 = 5;

impl Miner {
    pub async fn mine(&self, threads: u64) {
        // Register, if needed.
        let signer = self.signer();
        
        for wallet in 1..WALLETS+1 {
            self.register_by_number(wallet).await;    
        }        

        let mut stdout = stdout();
        let mut rng = rand::thread_rng();

        let mut reward_rate_sum = 0 as f64;
        let mut reward_rate_count = 0;
        let mut reward_rate_retries = 0;
        let mut last_reward_rate = 0 as f64;

        // Start mining loop
        loop {
            // Fetch account state
            let treasury = get_treasury(&self.rpc_client).await;
            let reward_rate = (treasury.reward_rate as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
            let priority_fee = (reward_rate * 10000000000.0) as u64;

            stdout.write_all(b"\x1b[2J\x1b[3J\x1b[H").ok();
            
            for wallet in 1..WALLETS+1 {
                let balance = self.get_ore_display_balance(wallet).await;
                println!("Balance: {} ORE", balance);
            }

            println!("Reward rate: {} ORE", reward_rate);
            println!("Using priority fee: {} lamports", priority_fee);
            
            println!("Avg Reward rate: {} ORE", reward_rate_sum as f64 / reward_rate_count as f64);
           
            //don't count same rate repeating
            if last_reward_rate as f64 != reward_rate {
                last_reward_rate = reward_rate;
                reward_rate_sum += reward_rate;
                reward_rate_count += 1;
            }

            //if reward less than average, retry a few times
            if reward_rate < (reward_rate_sum as f64 / reward_rate_count as f64) * 0.875 {
                if reward_rate_retries < 5 {
                    reward_rate_retries += 1;
                    std::thread::sleep(Duration::from_millis(4000));
                    continue;
                }
                else {
                    reward_rate_retries = 0;
                }
            }

            // test for mine speed
            /*
            for _ in 0..1000 {
                let proof = get_proof(&self.rpc_client, signer.pubkey()).await;
                let (next_hash, nonce) = self.find_next_hash_par(&self.signer_by_number(1), proof.hash.into(), treasury.difficulty.into(), threads as usize);
                println!("Next Hash {}", next_hash.to_string());
            } 
            */

            // Escape sequence that clears the screen and the scrollback buffer
            println!("\nMining for a valid hashes...");
            let mut next_hashes: HashMap<u64, KeccakHash> = HashMap::new();
            let mut nonces: HashMap<u64, u64> = HashMap::new();

            for wallet in 1..WALLETS+1 {
                println!("Wallet {} : {}", wallet, self.signer_by_number(wallet).pubkey().to_string());
                let proof = get_proof(&self.rpc_client, self.signer_by_number(wallet).pubkey()).await;
                println!("Proof Hash {} : {}", wallet, proof.hash.to_string());
                let rewards = (proof.claimable_rewards as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
                println!("Rewards Available {} : {}", wallet, rewards);
                let (next_hash, nonce) = self.find_next_hash_par(&self.signer_by_number(wallet), proof.hash.into(), treasury.difficulty.into(), threads as usize);
                println!("Next Hash {} : {}", wallet, next_hash.to_string());
                next_hashes.insert(wallet, next_hash);
                nonces.insert(wallet, nonce);
            }

            // Submit mine tx.
            // Use busses randomly so on each epoch, transactions don't pile on the same busses
            //println!("\n\nSubmitting hash for validation...");
            'submit: loop {
                // Double check we're submitting for the right challenge
                for wallet in 1..WALLETS+1 {
                    //println!("\nChecking hash already validated for wallet {}...", wallet);
                    let proof_ = get_proof(&self.rpc_client, self.signer_by_number(wallet).pubkey()).await;
                    if !self.validate_hash(
                        *next_hashes.get(&wallet).unwrap(),
                        proof_.hash.into(),
                        self.signer_by_number(wallet).pubkey(),
                        *nonces.get(&wallet).unwrap(),
                        treasury.difficulty.into(),
                    ) {
                        println!("{} Success: Hash already validated for wallet {}! An earlier transaction must have landed.", wallet, chrono::offset::Local::now());
                        break 'submit;
                    }
                }

                // Reset epoch, if needed
                let treasury = get_treasury(&self.rpc_client).await;
                let clock = get_clock_account(&self.rpc_client).await;
                let threshold = treasury.last_reset_at.saturating_add(EPOCH_DURATION);
                if clock.unix_timestamp.ge(&threshold) {
                    // There are a lot of miners right now, so randomly select into submitting tx
                    if rng.gen_range(0..RESET_ODDS).eq(&0) {
                        println!("Sending epoch reset transaction...");
                        let cu_limit_ix =
                            ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_RESET);
                        let cu_price_ix =
                            ComputeBudgetInstruction::set_compute_unit_price(self.priority_fee);
                        let reset_ix = ore::instruction::reset(signer.pubkey());
                        self.send_and_confirm(&[cu_limit_ix, cu_price_ix, reset_ix], false, true, vec![&signer])
                            .await
                            .ok();
                    }
                }

                // Submit request.
                let bus = self.find_bus_id(treasury.reward_rate).await;
                let bus_rewards = (bus.rewards as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
                println!("Sending on bus {} ({} ORE)", bus.id, bus_rewards);
                let cu_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_MINE);
                let cu_price_ix =
                    ComputeBudgetInstruction::set_compute_unit_price(priority_fee);
                
                let mut mine_ixs: Vec<Instruction> = Vec::new();
                mine_ixs.push(cu_limit_ix);
                mine_ixs.push(cu_price_ix);
                for wallet in 1..WALLETS+1 {
                    let ix_mine = ore::instruction::mine(
                        self.signer_by_number(wallet).pubkey(),
                        BUS_ADDRESSES[bus.id as usize],
                        (*next_hashes.get(&wallet).unwrap()).into(),
                        *nonces.get(&wallet).unwrap(),
                    );
                    mine_ixs.push(ix_mine);
                    //println!("Added mine txn for wallet {}", wallet);
                }

                //this is ugly but keypair doesn't implement clone() :(
                let signer1 = self.signer_by_number(1);
                let signer2 = self.signer_by_number(2);
                let signer3 = self.signer_by_number(3);
                let signer4 = self.signer_by_number(4);
                let signer5 = self.signer_by_number(5);

                let signers = vec![&signer1, &signer2, &signer3, &signer4, &signer5];

                match self
                    .send_and_confirm(&mine_ixs.into_boxed_slice(), false, false, signers)
                    .await
                {
                    Ok(sig) => {
                        println!("{} Success: {}", chrono::offset::Local::now(), sig);
                        break;
                    }
                    Err(_err) => {
                        // TODO
                    }
                }
            }
        }
    }

    async fn find_bus_id(&self, reward_rate: u64) -> Bus {
        let mut rng = rand::thread_rng();
        loop {
            let bus_id = rng.gen_range(0..BUS_COUNT);
            if let Ok(bus) = self.get_bus(bus_id).await {
                if bus.rewards.gt(&reward_rate.saturating_mul(20)) {
                    return bus;
                }
            }
        }
    }

    fn _find_next_hash(&self, hash: KeccakHash, difficulty: KeccakHash) -> (KeccakHash, u64) {
        let signer = self.signer();
        let mut next_hash: KeccakHash;
        let mut nonce = 0u64;
        loop {
            next_hash = hashv(&[
                hash.to_bytes().as_slice(),
                signer.pubkey().to_bytes().as_slice(),
                nonce.to_le_bytes().as_slice(),
            ]);
            if next_hash.le(&difficulty) {
                break;
            } else {
                println!("Invalid hash: {} Nonce: {:?}", next_hash.to_string(), nonce);
            }
            nonce += 1;
        }
        (next_hash, nonce)
    }

    fn find_next_hash_par(
        &self,
        signer: &Keypair,
        hash: KeccakHash,
        difficulty: KeccakHash,
        threads: usize,
    ) -> (KeccakHash, u64) {
        let start_time = Instant::now();     
        let found_solution = Arc::new(AtomicBool::new(false));
        let solution = Arc::new(Mutex::new((KeccakHash::new_from_array([0; 32]), 0)));
              
        println!("Difficulty is: {}", difficulty.to_string());

        thread::scope(|s| {
            for t in 0..threads {
                let found_solution = Arc::clone(&found_solution);
                let solution = Arc::clone(&solution);
                let n_min = u64::MAX.saturating_div(threads as u64).saturating_mul(t as u64);
                let n_max = n_min + u64::MAX.saturating_div(threads as u64);
                let pubkey = signer.pubkey();

                s.spawn(move |_| {
                    let mut counter = 0;
                    loop {
                        // just random guess, nothing fancy
                        let nonce = fastrand::u64(n_min..n_max);

                        let next_hash = hashv(&[
                            hash.as_ref(),
                            pubkey.as_ref(),
                            nonce.to_le_bytes().as_ref(),
                        ]);
                        
                        counter += 1;
                        if counter % 10_000 == 0 && found_solution.load(Ordering::Relaxed) {
                            return;
                        }

                        if next_hash.le(&difficulty) {
                            //println!("Correct guess is: {} by thread {}", nonce, t);
                            found_solution.store(true, Ordering::Relaxed);
                            let mut w_solution = solution.lock().expect("failed to lock mutex");
                            *w_solution = (next_hash, nonce);
                            return;
                        }
                    }
                });
            }
        })
        .unwrap();

        let elapsed_time = start_time.elapsed().as_millis();
        println!("Elapsed mine time: {}", elapsed_time);

        let r_solution = solution.lock().expect("Failed to get lock");
        *r_solution
    }

    pub fn validate_hash(
        &self,
        hash: KeccakHash,
        current_hash: KeccakHash,
        signer: Pubkey,
        nonce: u64,
        difficulty: KeccakHash,
    ) -> bool {
        // Validate hash correctness
        let hash_ = hashv(&[
            current_hash.as_ref(),
            signer.as_ref(),
            nonce.to_le_bytes().as_slice(),
        ]);

        //println!("Validating Hashes {} : {}", hash.to_string(), hash_.to_string());

        if sol_memcmp(hash.as_ref(), hash_.as_ref(), HASH_BYTES) != 0 {
            return false;
        }

        // Validate hash difficulty
        if hash.gt(&difficulty) {
            return false;
        }

        true
    }

    pub async fn get_ore_display_balance(&self, signer_number: u64) -> String {
        let client = self.rpc_client.clone();
        
        let signer = self.signer_by_number(signer_number);

        let token_account_address = spl_associated_token_account::get_associated_token_address(
            &signer.pubkey(),
            &ore::MINT_ADDRESS,
        );

        match client.get_token_account(&token_account_address).await {
            Ok(token_account) => {
                if let Some(token_account) = token_account {
                    token_account.token_amount.ui_amount_string
                } else {
                    "0.00".to_string()
                }
            }
            Err(_) => "0.00".to_string(),
        }
    }
}
