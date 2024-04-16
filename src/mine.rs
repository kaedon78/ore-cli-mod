use crossbeam::thread;
use std::{
    io::{stdout, Write},
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::{Duration, UNIX_EPOCH},
    env,
    time::Instant,
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
use tokio::io::AsyncWriteExt;
use crate::{
    cu_limits::{CU_LIMIT_MINE},
    utils::{get_clock_account, get_proof, get_treasury},
    Miner,
};
use chrono::{Local};
use chrono::prelude::DateTime;

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
    pub async fn mine(&self, _threads: u64) {

        /*let input = b"Hello, CUDA!";
        let digest_size = 256; // Choose the digest size (in bits)
        let mut output = vec![0u8; digest_size / 8];
        unsafe {
            cuda_wrapper::keccakHash(
                input.as_ptr(),
                output.as_mut_ptr(),
                digest_size as u32,
            );
        }
        println!("Keccak Hash: {:x?}", output);
        */

        // Register, if needed.
        for wallet in 1..WALLETS+1 {
            self.register_by_number(wallet).await;    
        }        

        let mut stdout = stdout();

        let mut reward_rate_sum = 0 as f64;
        let mut reward_rate_count = 0;
        let mut last_reward_rate = 0 as f64;
        let mut last_submit_time = 0;
        let mut total_times_mined = 0;
        let mut total_mining_mills = 0;
        let mut total_submit_mills = 0;

        // Start mining loop
        loop {
            stdout.write_all(b"\x1b[2J\x1b[3J\x1b[H").ok();
            if last_submit_time > 0 {
                println!("Last reward took {} seconds to land\n", last_submit_time/1000);
            }

            // Fetch account state
            let treasury = get_treasury(&self.rpc_client).await;
            println!("Treasury Difficulty: {}", treasury.difficulty.to_string());
            
            let d = UNIX_EPOCH + Duration::from_secs(treasury.last_reset_at.try_into().unwrap());
            let datetime = DateTime::<Local>::from(d);
            let timestamp_str = datetime.format("%Y-%m-%d %H:%M:%S").to_string();
            println!("Treasury Last reset at: {}", timestamp_str);
            
            let threshold = treasury.last_reset_at.saturating_add(EPOCH_DURATION);
            let d2 = UNIX_EPOCH + Duration::from_secs(threshold.try_into().unwrap());
            let datetime2 = DateTime::<Local>::from(d2);
            let timestamp_str2 = datetime2.format("%Y-%m-%d %H:%M:%S").to_string();
            println!("Treasury Next reset at: {}", timestamp_str2);

            let reward_rate = (treasury.reward_rate as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
            
            let priority_fee = self.priority_fee;
            
            //println!("Main wallet balance: {} ORE", self.get_ore_display_balance(1).await);

            println!("Current reward rate: {} ORE", reward_rate);
            println!("Using priority fee: {} micro-lamports", priority_fee);
            println!("Avg reward rate: {} ORE", reward_rate_sum as f64 / reward_rate_count as f64);
            if total_times_mined > 0 {
                println!("Total txns: {}", total_times_mined);
                println!("Avg time per txn: {} seconds", (total_submit_mills+total_mining_mills) / total_times_mined / 1000);
            }
           
            //don't count same rate repeating
            if last_reward_rate as f64 != reward_rate {
                last_reward_rate = reward_rate;
                reward_rate_sum += reward_rate;
                reward_rate_count += 1;
            }

            //if reward less than average, retry a few times
            /*
            if reward_rate < (reward_rate_sum as f64 / reward_rate_count as f64) * 0.875 {
                println!("Current reward rate less than average, waiting a few more seconds...");
                if reward_rate_retries < 3 {
                    reward_rate_retries += 1;
                    std::thread::sleep(Duration::from_millis(3000));
                    continue;
                }
                else {
                    reward_rate_retries = 0;
                }
            }
            */

            // test for mine speed
            /*
            for _ in 0..1000 {
                let proof = get_proof(&self.rpc_client, signer.pubkey()).await;
                let (next_hash, nonce) = self.find_next_hash_par(&self.signer_by_number(1), proof.hash.into(), treasury.difficulty.into(), threads as usize);
                println!("Next Hash {}", next_hash.to_string());
            } 
            */

            // Escape sequence that clears the screen and the scrollback buffer
            println!("\nMining for valid hashes...");
            //let mut next_hashes: HashMap<u64, KeccakHash> = HashMap::new();
            //let mut nonces: HashMap<u64, u64> = HashMap::new();

            let mut all_pubkey = vec![];
            let mut proofs = vec![];
            for wallet in 1..WALLETS+1 {
                let proof = get_proof(&self.rpc_client, self.signer_by_number(wallet).pubkey()).await;
                //println!("Proof Hash {} : {}", wallet, proof.hash.to_string());
                let rewards = (proof.claimable_rewards as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
                println!("Wallet {} claimable rewards: {} ORE", wallet, rewards);
                
                //let (next_hash, nonce) = self.find_next_hash_par(&self.signer_by_number(wallet), proof.hash.into(), treasury.difficulty.into(), threads);
                
                all_pubkey.push(self.signer_by_number(wallet).pubkey().clone());
                proofs.push(proof);
            }

            let hash_and_pubkey = all_pubkey
                .iter()
                .zip(proofs.iter())
                .map(|(signer, proof)| (solana_sdk::keccak::Hash::new_from_array(proof.hash.0), *signer))
                .collect::<Vec<_>>();

            let start_time = Instant::now();
            println!("Mining initial {} hashes started...", hash_and_pubkey.len());
            let mut mining_result = self.find_next_hash_par_gpu(&treasury.difficulty.into(), &hash_and_pubkey, 0).await;
            println!("Mining initial {} hashes took {} sec", hash_and_pubkey.len(), start_time.elapsed().as_millis()/1000);
            
            println!("Checking all hashes are valid...");
            for wallet in 1..WALLETS+1 {
                //println!("{}, {}, {}, {}", treasury.difficulty.to_string(), proof.hash.to_string(), self.signer_by_number(wallet).pubkey().to_string(), hash_and_pubkey.len());
                let result_idx: usize = (wallet-1) as usize;
                let mut lastproof = mining_result[result_idx].2;
                let currentproof = get_proof(&self.rpc_client, self.signer_by_number(wallet).pubkey()).await;
                loop {
                    if lastproof == currentproof.hash.into() {
                        break;
                    }
                    
                    println!("Proof changed for wallet {}, re-mining...", wallet);
                    let hash_and_pubkey = [(solana_sdk::keccak::Hash::new_from_array(currentproof.hash.0), self.signer_by_number(wallet).pubkey().clone())];
                    let result = self.find_next_hash_par_gpu(&treasury.difficulty.into(), &hash_and_pubkey, 0).await;
                    //proof = get_proof(&self.rpc_client, self.signer_by_number(wallet).pubkey()).await;
                    let valid_result = self.validate_hash(
                        result[0].0,
                        currentproof.hash.into(),
                        self.signer_by_number(wallet).pubkey().clone(),
                        result[0].1,
                        treasury.difficulty.into(),
                    );

                    //println!("Validated with params {}, {}, {}, {}, {}", result[0].0, result[0].1, result[0].2, self.signer_by_number(wallet).pubkey(), treasury.difficulty);

                    lastproof = currentproof.hash.into();

                    if valid_result {
                        mining_result[result_idx] = result[0];
                        break; 
                    }
                }

                //println!("Next Hash {} : {}", result.0, result.1);
                //next_hashes.insert(wallet, result.0);
                //nonces.insert(wallet, result.1);
                
                //next_hashes.insert(wallet, next_hash);
                //nonces.insert(wallet, nonce);
            }
            /*
            let hash_and_pubkey = all_pubkey
                .iter()
                .zip(proofs.iter())
                .map(|(signer, proof)| (solana_sdk::keccak::Hash::new_from_array(proof.hash.0), *signer))
                .collect::<Vec<_>>();
            */
            
            //println!("{}, {}, {}, {}", treasury.difficulty.to_string(), proof.hash.to_string(), self.signer_by_number(wallet).pubkey().to_string(), hash_and_pubkey.len());
            //let start_time = Instant::now();
            
            //if (self.use_gpu) {
                //let mining_result = self.find_next_hash_par_gpu(&treasury.difficulty.into(), &hash_and_pubkey, 0).await;
            //}
            //else {
                //for wallet in 1..WALLETS+1 {
                    //let (next_hash, nonce) = self.find_next_hash_par(&self.signer_by_number(wallet), proof.hash.into(), treasury.difficulty.into(), threads);
                   //}
            //}

            let this_mine_time = start_time.elapsed().as_millis();
            println!("This hash mining time: {} sec", this_mine_time/1000);
            if total_times_mined > 0 {
                println!("Avg hash mining time: {} sec", total_mining_mills/total_times_mined/1000);
            }
            //std::thread::sleep(Duration::from_millis(3000));

            // Submit mine tx.
            // Use busses randomly so on each epoch, transactions don't pile on the same busses
            //println!("\n\nSubmitting hash for validation...");
             let start_time_submit = Instant::now();                 
            'submit: loop {
                let treasury = get_treasury(&self.rpc_client).await;

                //println!("Validating Hashes...");
                // Double check we're submitting for the right challenge
                for wallet in 1..WALLETS+1 {
                    //println!("\nChecking hash already validated for wallet {}...", wallet);
                    let proof_ = get_proof(&self.rpc_client, self.signer_by_number(wallet).pubkey()).await;
                    let result_idx: usize = (wallet-1) as usize;
                    
                    //println!("Validaing with params {}, {}, {}, {}, {}", mining_result[result_idx].0, mining_result[result_idx].1, proof_.hash, self.signer_by_number(wallet).pubkey(), treasury.difficulty);
                    
                    if mining_result[result_idx].2 != proof_.hash.into() {
                        println!("Hash already validated for wallet {}", wallet);
                    }

                    if !self.validate_hash(
                        mining_result[result_idx].0,
                        proof_.hash.into(),
                        self.signer_by_number(wallet).pubkey(),
                        mining_result[result_idx].1,
                        treasury.difficulty.into(),
                    ) {
                        println!("Hash already validated for wallet {}", wallet);
                        break 'submit;
                    }
                }

                // Reset epoch, if needed
                loop {
                    //println!("Checking Epoch Reset...");
                    let treasury = get_treasury(&self.rpc_client).await;
                    let clock = get_clock_account(&self.rpc_client).await;
                    let epoch_valid = self.check_epoch_reset(&treasury, &clock).await;
                    if epoch_valid {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1000));
                }
                let epoch_threshold = treasury.last_reset_at.saturating_add(EPOCH_DURATION) as u64;

                /*
                let treasury = get_treasury(&self.rpc_client).await;
                //println!("Calling getaccount for clock");
                let clock = get_clock_account(&self.rpc_client).await;
                let threshold = treasury.last_reset_at.saturating_add(EPOCH_DURATION);
                if clock.unix_timestamp.ge(&threshold) {
                    // There are a lot of miners right now, so randomly select into submitting tx
                    if rng.gen_range(0..RESET_ODDS).eq(&0) {
                        println!("\nSending epoch reset transaction...");
                        let cu_limit_ix =
                            ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_RESET);
                        let cu_price_ix =
                            ComputeBudgetInstruction::set_compute_unit_price(priority_fee);
                        let reset_ix = ore::instruction::reset(signer.pubkey());
                        self.send_and_confirm(&[cu_limit_ix, cu_price_ix, reset_ix], false, true, vec![&signer])
                            .await
                            .ok();
                    }
                }
                */

                // Submit request.
                let bus = self.find_bus_id(treasury.reward_rate).await;
                let bus_rewards = (bus.rewards as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
                println!("\nSending on bus {} ({} ORE)", bus.id, bus_rewards);
                let cu_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT_MINE);
                let cu_price_ix =
                    ComputeBudgetInstruction::set_compute_unit_price(priority_fee);
                
                let mut mine_ixs: Vec<Instruction> = Vec::new();
                mine_ixs.push(cu_limit_ix);
                mine_ixs.push(cu_price_ix);
                for wallet in 1..WALLETS+1 {
                    let result_idx: usize = (wallet-1) as usize;    
                    let ix_mine = ore::instruction::mine(
                        self.signer_by_number(wallet).pubkey(),
                        BUS_ADDRESSES[bus.id as usize],
                        mining_result[result_idx].0.into(),
                        mining_result[result_idx].1,
                        //(*next_hashes.get(&wallet).unwrap()).into(),
                        //*nonces.get(&wallet).unwrap(),
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
                    .send_and_confirm(&mine_ixs.into_boxed_slice(), false, false, signers, epoch_threshold)
                    .await
                {
                    Ok(sig) => {
                        println!("{} Success: {}", chrono::offset::Local::now(), sig);

                        last_submit_time = start_time_submit.elapsed().as_millis();
                        total_submit_mills += last_submit_time;

                        total_times_mined += 1;
                        total_mining_mills += this_mine_time;

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

    fn _find_next_hash_par(
        &self,
        signer: &Keypair,
        hash: KeccakHash,
        difficulty: KeccakHash,
        threads: u64,
    ) -> (KeccakHash, u64) {
        let found_solution = Arc::new(AtomicBool::new(false));
        let solution = Arc::new(Mutex::new((KeccakHash::new_from_array([0; 32]), 0)));
        let pubkey = signer.pubkey();
        let work_per_thread = u64::MAX / threads;
    
        thread::scope(|s| {
            for t in 0..threads {
                let found_solution = Arc::clone(&found_solution);
                let solution = Arc::clone(&solution);
                let start_nonce = t * work_per_thread;
                let end_nonce = start_nonce + work_per_thread;
                s.spawn(move |_| {
                    for nonce in start_nonce..end_nonce {
                        if nonce % 100_000 == 0 && found_solution.load(Ordering::Relaxed) {
                            break;
                        }
                        let next_hash = hashv(&[
                            hash.as_ref(),
                            pubkey.as_ref(),
                            nonce.to_le_bytes().as_ref(),
                        ]);
                        if next_hash <= difficulty {
                            found_solution.store(true, Ordering::Relaxed);
                            let mut sol = solution.lock().unwrap();
                            *sol = (next_hash, nonce);
                            break;
                        }
                    }
                });
            }
        }).unwrap();
    
        let r_solution = solution.lock().expect("Failed to get lock");
        *r_solution
    }

    async fn  find_next_hash_par_gpu(
        &self,
        difficulty: &solana_sdk::keccak::Hash,
        hash_and_pubkey: &[(solana_sdk::keccak::Hash, Pubkey)],
        threads: usize
    ) -> Vec<(KeccakHash, u64, KeccakHash)> {
        let mut child = tokio::process::Command::new(env::current_exe().unwrap().parent().unwrap().join("gpu-worker"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("worker failed to spawn");
 
        if let Some(mut stdin) = child.stdin.take() {
            let mut data_to_send = Vec::new();
 
            // Add difficulty bytes
            data_to_send.extend_from_slice(difficulty.as_ref());
 
            // Add hash and pubkey bytes
            for (hash, pubkey) in hash_and_pubkey {
                data_to_send.extend_from_slice(hash.as_ref());
                data_to_send.extend_from_slice(pubkey.as_ref());
            }
 
            // Optionally prepend the number of threads or any other control data
            // Here, we send the number of threads as the first byte, if required by your application
            let mut final_data = Vec::new();
            final_data.push(threads as u8);
            final_data.extend_from_slice(&data_to_send);
 
            // Write all bytes in one go
            stdin.write_all(&final_data).await.unwrap();
 
            // Dropping stdin to close it, signaling the end of input
            drop(stdin);
        }

        let output = child.wait_with_output().await.unwrap().stdout;
        let mut results = vec![];
        //println!("output {:?}", output);
        let mut outidx = 0;
        let chunks = output.chunks(40);
        for chunk in chunks {
            if chunk.len() < 40 {
                //println!("Incomplete data chunk received, length: {}", chunk.len());
                continue;  // Skip this chunk or handle it according to your needs
            }
            let proof = hash_and_pubkey[outidx].0.into();
            let hash = solana_sdk::keccak::Hash(chunk[..32].try_into().unwrap());
            let nonce = u64::from_le_bytes(chunk[32..40].try_into().unwrap());
           // println!("hash {:?}", hash);
            //println!("nonce {:?}", nonce);
            results.push((hash, nonce, proof));
            outidx += 1;
        }
        //println!("{:?}", results);
  
        results
            /*
            .cloned()
            .ok_or_else(|| "No valid results were found".to_string())
            .expect("REASON")
            */
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
            println!("Invalid Hash {} : {}", hash.to_string(), hash_.to_string());    
            return false;
        }

        // Validate hash difficulty
        if hash.gt(&difficulty) {
            return false;
        }

        true
    }

    pub async fn _get_ore_display_balance(&self, signer_number: u64) -> String {
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
