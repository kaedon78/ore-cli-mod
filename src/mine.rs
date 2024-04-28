#[cfg(feature = "ore")]
use ore::{self, instruction, state::Bus, TOKEN_DECIMALS, MINT_ADDRESS, BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION};
#[cfg(feature = "orz")]
use orz::{self, instruction, state::Bus, TOKEN_DECIMALS, MINT_ADDRESS, BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION};
#[cfg(feature = "mars")]
use mars::{self, instruction, state::Bus, TOKEN_DECIMALS, MINT_ADDRESS, BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION};

use crossbeam::{
    thread,
    channel
};
use std::{
    io::{stdout, Write},
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::Duration,
    env,
    time::Instant,
    fs, 
    fs::File
};
use rand::Rng;
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
    constants::{CU_LIMIT_MINE, TOKEN_NAME, GPU_SYNC_FILE},
    utils::{get_clock_account, get_proof, get_treasury},
    Miner,
};

impl Miner {
    pub fn payer(&self) -> &Keypair {
        if self.keypair_fee.is_some()  {
            &self.keypair_fee.as_ref().unwrap()
        }
        else {
            &self.wallets[0]
        }
    }    

    pub async fn mine(&self) {
        // Register, if needed.
        self.register_all().await;
        let mut last_bus_id: u64 = 0;
        let mut stdout = stdout();
        let mut test_mode = false;
        let mut test_diff_array: [u8;32] = [0, 0, 0, 16, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255];

        // Start mining loop
        loop {
            // Escape sequence that clears the screen and the scrollback buffer    
            stdout.write_all(b"\x1b[2J\x1b[3J\x1b[H").ok();

            {
                let stats = self.stats.borrow_mut();
                if stats.last_submit_time > 0 {
                    println!("Last reward took {}s to land using GATEWAY_DELAY of {}ms\n", stats.last_submit_time/1000, stats.gateway_delay_adj);
                }
                stats.print_api_calls();
            }            

            // Fetch account state
            self.stats.borrow_mut().add_api_call("getaccountinfo");
            let mut treasury = get_treasury(&self.rpc_client).await;
            self.stats.borrow_mut().add_api_call("getaccountinfo");
            let mut clock:solana_program::clock::Clock;

            let mut mining_difficulty = treasury.difficulty;
            
            //test mode - difficulty check
            if mining_difficulty.to_string() == "11111111111111111111111111111111" {
                test_mode = true;
                mining_difficulty = KeccakHash::new_from_array(test_diff_array).into();

                {
                    let stats = self.stats.borrow_mut();
                    if stats.total_times_submitted > 0 {
                        let avg_mine_time_sec = stats.total_mining_mills / stats.total_times_submitted as u128 / 1000;
                        if avg_mine_time_sec < 40 && stats.last_mine_time  < 50000 {
                            test_diff_array = self.get_next_difficulty(test_diff_array, 1);
                            mining_difficulty = KeccakHash::new_from_array(test_diff_array).into();
                            //stats.reset_stats();
                        }
                        else if avg_mine_time_sec > 50 || stats.last_mine_time  > 60000 {
                            test_diff_array = self.get_next_difficulty(test_diff_array, -1);
                            mining_difficulty = KeccakHash::new_from_array(test_diff_array).into();
                            //stats.reset_stats();
                        }
                    }
                }
                println!("Mining paused, using test difficulty: [{},{}] : {}", test_diff_array[3], test_diff_array[4], mining_difficulty.to_string());    
            }            

            let reward_rate = self.get_reward_rate(&treasury);
            let priority_fee = self.priority_fee;
            
            //println!("Main wallet balance: {} {}", self.get_ore_display_balance(&self.wallets[0].pubkey()).await, TOKEN_NAME);
            {
                let mut stats = self.stats.borrow_mut();
                stats.print_stats();
                stats.update_avg_reward_rate(reward_rate);
            }

            println!("Using priority fee: {} micro-lamports", priority_fee);
            println!("Current reward rate: {} {}", reward_rate, TOKEN_NAME);

            println!("\nMining for valid hashes...");
            let mut all_pubkey = vec![];
            let mut proofs = vec![];
            for w in 0..self.wallets.len() {
                self.stats.borrow_mut().add_api_call("getaccountinfo");
                let proof = get_proof(&self.rpc_client, self.wallets[w].pubkey()).await;
                let rewards = (proof.claimable_rewards as f64) / (10f64.powf(TOKEN_DECIMALS as f64));
                println!("Wallet {} claimable rewards: {} {}", w, rewards, TOKEN_NAME);
                
                //let (next_hash, nonce) = self.find_next_hash_par(&self.signer_by_number(wallet), proof.hash.into(), treasury.difficulty.into(), threads);
                
                all_pubkey.push(self.wallets[w].pubkey().clone());
                proofs.push(proof);
            }

            let hash_and_pubkey = all_pubkey
                .iter()
                .zip(proofs.iter())
                .map(|(signer, proof)| (solana_sdk::keccak::Hash::new_from_array(proof.hash.0), *signer))
                .collect::<Vec<_>>();

            
            if self.use_gpu {
                // Check if the sync file exists

                println!("Waiting on GPU to be free...");
                while fs::metadata(GPU_SYNC_FILE).is_ok() {
                    std::thread::sleep(Duration::from_millis(250));
                }
            
                let mut _file = File::create(GPU_SYNC_FILE);
            }

            let start_mine_time = Instant::now();
            println!("Mining initial {} hashes started...", hash_and_pubkey.len());
           
            let mut mining_result = self.find_next_hash_decider(&mining_difficulty.into(), &hash_and_pubkey, self.threads).await;
            println!("Mining initial {} hashes took {} sec", hash_and_pubkey.len(), start_mine_time.elapsed().as_millis()/1000);
            
            println!("Checking all hashes are valid...");
            self.validate_hashes(&mining_difficulty.into(), &mut mining_result, test_mode).await;

            if self.use_gpu {
                // Remove the sync file
                let _ = fs::remove_file(GPU_SYNC_FILE);
            }

            {
                self.stats.borrow_mut().record_mine(start_mine_time);
            }

            // Submit mine tx.
            // Use busses randomly so on each epoch, transactions don't pile on the same busses
            //println!("\n\nSubmitting hash for validation...");
             let start_time_submit = Instant::now();
            'submit: loop {
                //println!("Validating Hashes...");
                // Double check we're submitting for the right challenge
                for w in 0..self.wallets.len() {
                    //println!("\nChecking hash already validated for wallet {}...", wallet);
                    self.stats.borrow_mut().add_api_call("getaccountinfo");
                    let proof_ = get_proof(&self.rpc_client, self.wallets[w].pubkey()).await;
                    
                    //println!("Validaing with params {}, {}, {}, {}, {}", mining_result[result_idx].0, mining_result[result_idx].1, proof_.hash, self.signer_by_number(wallet).pubkey(), treasury.difficulty);
                    
                    if mining_result[w].2 != proof_.hash.into() {
                        println!("Hash invalid for wallet {}", w);
                    }

                    if !self.validate_hash(
                        mining_result[w].0,
                        proof_.hash.into(),
                        self.wallets[w].pubkey(),
                        mining_result[w].1,
                        mining_difficulty.into(),
                    ) {
                        println!("Hash invalid for wallet {}", w);
                        break 'submit;
                    }
                }

                // Reset epoch, if needed
                loop {
                    self.stats.borrow_mut().add_api_call("getaccountinfo");
                    treasury = get_treasury(&self.rpc_client).await;
                    self.stats.borrow_mut().add_api_call("getaccountinfo");
                    clock = get_clock_account(&self.rpc_client).await;
   
                    //println!("Checking Epoch Reset...");
                    let epoch_valid = self.check_epoch_reset(&treasury, &clock).await;
                    if epoch_valid {
                        break;
                    }
                    else {
                        self.wait_for_next_epoch(&treasury, &clock).await;
                    }
                }

                // Submit request.
                let bus = self.find_bus_id(treasury.reward_rate, last_bus_id).await;
                last_bus_id = bus.id;
                let bus_rewards = (bus.rewards as f64) / (10f64.powf(TOKEN_DECIMALS as f64));
                println!("\nSending on bus {} ({} {})", bus.id, bus_rewards, TOKEN_NAME);
                let cu_limit_amt = (CU_LIMIT_MINE * self.wallets.len() as u32) + 500;
                let cu_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(cu_limit_amt);
                let cu_price_ix =
                    ComputeBudgetInstruction::set_compute_unit_price(priority_fee);
                
                let mut mine_ixs: Vec<Instruction> = Vec::new();
                mine_ixs.push(cu_limit_ix);
                mine_ixs.push(cu_price_ix);
                for w in 0..self.wallets.len() {
                    let ix_mine = instruction::mine(
                        self.wallets[w].pubkey(),
                        BUS_ADDRESSES[bus.id as usize],
                        mining_result[w].0.into(),
                        mining_result[w].1,
                        //(*next_hashes.get(&wallet).unwrap()).into(),
                        //*nonces.get(&wallet).unwrap(),
                    );
                    mine_ixs.push(ix_mine);
                    //println!("Added mine txn for wallet {}", wallet);
                }
                
                if test_mode {
                    println!("{} Txn success.. (but not really, just testing)", chrono::offset::Local::now());
                    std::thread::sleep(Duration::from_millis(1000));
                    self.stats.borrow_mut().record_submit(start_time_submit);
                    break;
                }

                let epoch_threshold = treasury.last_reset_at.saturating_add(EPOCH_DURATION) as u64;

                let mut signers: Vec<&Keypair> = self.wallets.iter().collect();
                if self.payer() != &self.wallets[0] {
                    signers.insert(0, self.payer());
                }
                
                match self
                    .send_and_confirm(&mine_ixs.into_boxed_slice(), false, false, signers, epoch_threshold)
                    .await
                {
                    Ok(sig) => {
                        println!("{} Txn success: {}", chrono::offset::Local::now(), sig);
                        self.stats.borrow_mut().record_submit(start_time_submit);
                        break;
                    }
                    Err(err) => {
                        if err.to_string().contains("Epoch reset") {
                            //self.wait_for_next_epoch(&treasury, &clock).await;    
                        }
                        else if err.to_string().contains("Blockhash not found") {
                            //nothing to do here
                        }
                        else if err.to_string().contains("This transaction has already been processed") {
                            self.stats.borrow_mut().record_submit(start_time_submit);    
                        }
                        else {
                            println!("{} Txn error: {}", chrono::offset::Local::now(), err.to_string());
                            std::thread::sleep(Duration::from_millis(1000));
                        }
                    }
                }
            }
        }
    }

    async fn validate_hashes(&self, mining_difficulty:&solana_sdk::keccak::Hash, mining_result: &mut Vec<(KeccakHash, u64, KeccakHash)>, test_mode: bool) {
        for w in 0..self.wallets.len() {
            //println!("{}, {}, {}, {}", test_difficulty.to_string(), lastproof.to_string(), self.wallets[w].pubkey().to_string(), hash_and_pubkey.len());
            let mut lastproof = mining_result[w].2;
            loop {
                let mut currentproof: KeccakHash = lastproof;
                if !test_mode {
                    self.stats.borrow_mut().add_api_call("getaccountinfo");
                    currentproof = get_proof(&self.rpc_client, self.wallets[w].pubkey()).await.hash.into();
                }
                if lastproof == currentproof {
                    break;
                }
                //println!("Proofs don't match for {}: {}, {}", w, lastproof, currentproof);
                //std::thread::sleep(Duration::from_millis(2000));
                    
                println!("Proof changed for wallet {}, re-mining...", w);
                let test_hash_and_pubkey = [(currentproof, self.wallets[w].pubkey().clone())];

                let result = self.find_next_hash_decider(mining_difficulty, &test_hash_and_pubkey, self.threads).await;

                let valid_result = self.validate_hash(
                    result[0].0,
                    currentproof,
                    self.wallets[w].pubkey().clone(),
                    result[0].1,
                    *mining_difficulty,
                );

                if valid_result {
                    lastproof = currentproof;
                    //println!("Validated with params {}, {}, {}, {}, {}", result[0].0, result[0].1, result[0].2, self.wallets[w].pubkey(), mining_difficulty);
                    mining_result[w] = result[0];
                }
                else {
                    //???
                }
            }
            //println!("Next Hash {} : {}", result.0, result.1);
        }    
    }

    fn get_next_difficulty(&self, start_array: [u8; 32], direction: i8) -> [u8; 32] {
        let mut diff_array = start_array;
        for x in 0..start_array.len() {
            if diff_array[x] != 0 {
                //less is more
                if direction > 0 {
                    diff_array[x] = diff_array[x].saturating_sub(8);
                }
                else {
                    diff_array[x] = diff_array[x].saturating_add(8);
                }
                break;
            }
        }
        diff_array
    }

    async fn find_next_hash_decider(&self, 
        difficulty: &solana_sdk::keccak::Hash,
        hash_and_pubkey: &[(solana_sdk::keccak::Hash, Pubkey)],
        threads: u64    
    ) -> Vec<(KeccakHash, u64, KeccakHash)> {
        if self.use_gpu {
            self.find_next_hash_par_gpu(difficulty, hash_and_pubkey, threads).await
        }
        else {
            self.find_next_hash_cpu(difficulty, hash_and_pubkey, threads)
        }
    }

    async fn find_bus_id(&self, reward_rate: u64, last_bus_id: u64) -> Bus {
        let mut rng = rand::thread_rng();
        loop {
            let bus_id = rng.gen_range(0..BUS_COUNT);
            if bus_id as u64 != last_bus_id {
                if let Ok(bus) = self.get_bus(bus_id).await {
                    //if bus.rewards.gt(&reward_rate.saturating_mul(20)) {
                    if bus.rewards.gt(&reward_rate.saturating_mul(0)) {
                        return bus;
                    }
                }
            }
        }
    }

    fn _find_next_hash(&self, hash: KeccakHash, difficulty: KeccakHash) -> (KeccakHash, u64) {
        let mut next_hash: KeccakHash;
        let mut nonce = 0u64;
        loop {
            next_hash = hashv(&[
                hash.to_bytes().as_slice(),
                self.payer().pubkey().to_bytes().as_slice(),
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

    fn find_next_hash_cpu(
        &self,
        difficulty: &KeccakHash,
        hash_and_pubkey: &[(solana_sdk::keccak::Hash, Pubkey)],
        threads: u64,
    ) -> Vec<(KeccakHash, u64, KeccakHash)> {
        let mut results = vec![];
        let work_per_thread = u64::MAX / threads;
    
        for (hash, pubkey) in hash_and_pubkey {
            let (tx, rx) = channel::unbounded();
            let found_solution = Arc::new(AtomicBool::new(false));
            let solution = Arc::new(Mutex::new((KeccakHash::new_from_array([0; 32]), 0, KeccakHash::new_from_array([0; 32]))));
            thread::scope(|s| {
                for t in 0..threads {
                    let tx = tx.clone();
                    let found_solution = Arc::clone(&found_solution);
                    let solution = Arc::clone(&solution);
                    let start_nonce = t * work_per_thread;
                    let end_nonce = start_nonce + work_per_thread;
                    s.spawn(move |_| {
                        for nonce in start_nonce..end_nonce {
                            if nonce % 100_000 == 0 && found_solution.load(Ordering::Relaxed) {
                                tx.send(()).unwrap(); 
                                break;
                            }
                            let next_hash = hashv(&[
                                hash.as_ref(),
                                pubkey.as_ref(),
                                nonce.to_le_bytes().as_ref(),
                            ]);
                            if next_hash <= *difficulty {
                                found_solution.store(true, Ordering::Relaxed);
                                let mut sol = solution.lock().unwrap();
                                *sol = (next_hash, nonce, hash.clone());
                                tx.send(()).unwrap(); // Notify completion if found
                                break;
                            }
                        }
                        tx.send(()).unwrap(); // Notify completion if not found
                    });
                }
            }).unwrap();
            
            for _ in 0..threads {
                rx.recv().unwrap(); // Wait for threads to complete
            }
            
            let r_solution = solution.lock().expect("Failed to get lock");
            results.push(r_solution.clone());
        }

        results
    }

    async fn find_next_hash_par_gpu(
        &self,
        difficulty: &KeccakHash,
        hash_and_pubkey: &[(KeccakHash, Pubkey)],
        _threads: u64
    ) -> Vec<(KeccakHash, u64, KeccakHash)> {
        let mut child = tokio::process::Command::new(env::current_exe().unwrap().parent().unwrap().join("gpu-worker"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("Worker failed to spawn... check gpu-worker.exe exists in target\\release path");
 
        if let Some(mut stdin) = child.stdin.take() {
            let mut data_to_send = Vec::new();
 
            // Add difficulty bytes
            data_to_send.extend_from_slice(difficulty.as_ref());
 
            // Add hash and pubkey bytes
            for (hash, pubkey) in hash_and_pubkey {
                data_to_send.extend_from_slice(hash.as_ref());
                data_to_send.extend_from_slice(pubkey.as_ref());
            }
 
            let mut final_data = Vec::new();
            final_data.push(0 as u8);
            final_data.extend_from_slice(&data_to_send);
 
            // Write all bytes in one go
            stdin.write_all(&final_data).await.unwrap();
            // Dropping stdin to close it, signaling the end of input
            drop(stdin);
        }

        let output = child.wait_with_output().await.unwrap().stdout;

        let mut results = vec![];
        let mut outidx = 0;
        let chunks = output.chunks(40);
        for chunk in chunks {
            if chunk.len() < 40 {
                continue;  // Skip this chunk or handle it according to your needs
            }
            let proof = hash_and_pubkey[outidx].0.into();
            let hash = solana_sdk::keccak::Hash(chunk[..32].try_into().unwrap());
            let nonce = u64::from_le_bytes(chunk[32..40].try_into().unwrap());
            results.push((hash, nonce, proof));
            outidx += 1;
        }
  
        results
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

    pub async fn _get_ore_display_balance(&self, address:&Pubkey) -> String {
        let client = self.rpc_client.clone();
        
        let token_account_address = spl_associated_token_account::get_associated_token_address(
            address,
            &MINT_ADDRESS,
        );
        
        self.stats.borrow_mut().add_api_call("getaccountinfo");
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
