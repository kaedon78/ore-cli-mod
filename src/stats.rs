use std::{
    time::Instant,
};

const TARGET_RATE_MULT: f64 = 0.875;

pub struct MinerStats {
	pub reward_rate_total: f64,
	pub reward_rate_count: u64,
	pub last_reward_rate: f64,
	pub last_submit_time: u128,
	pub total_times_submitted: u64,
	pub total_submit_mills: u128,
	pub total_mining_mills: u128,
	pub last_mine_time: u128,
}

impl MinerStats {
    pub fn new() -> Self {
        Self {
            reward_rate_total: 0.0,
			reward_rate_count: 0,
			last_reward_rate: 0.0,
			
			total_times_submitted: 0,
			
			total_submit_mills: 0,
			last_submit_time: 0,
			
			total_mining_mills: 0,
			last_mine_time: 0,
        }
    }

	pub fn print_stats(&self) {
        if self.total_times_submitted > 0 {
            println!("Avg hash mining time: {} sec", self.total_mining_mills / self.total_times_submitted as u128 / 1000);
        }
		if self.total_times_submitted > 0 {
			println!("Avg reward rate: {} ORE", self.reward_rate_total as f64 / self.reward_rate_count as f64);
			println!("Total txns: {}", self.total_times_submitted);
			println!("Avg time per txn: {} seconds", (self.total_submit_mills + self.total_mining_mills) / self.total_times_submitted as u128 / 1000);
		}
	}

	pub fn update_avg_reward_rate(&mut self, reward_rate: f64) {
		//don't count same rate repeating
        if self.last_reward_rate as f64 != reward_rate {
            self.last_reward_rate = reward_rate;
            self.reward_rate_total += reward_rate;
            self.reward_rate_count += 1;
        }	
	}

	pub fn is_reward_rate_above_avg(&self, reward_rate: f64) -> bool {
		reward_rate < (self.reward_rate_total as f64 / self.reward_rate_count as f64) * TARGET_RATE_MULT
	}

	pub fn record_mine(&mut self, start_time_mine:Instant) {
		self.last_mine_time = start_time_mine.elapsed().as_millis();
		self.total_mining_mills += self.last_mine_time;	
	}

	pub fn record_submit(&mut self, start_time_submit:Instant) {
		self.last_submit_time = start_time_submit.elapsed().as_millis();
		self.total_submit_mills += self.last_submit_time;
		self.total_times_submitted += 1;
	}

	pub fn reset_stats(&mut self) {
		self.reward_rate_total = 0.0;
		self.reward_rate_count = 0;
		self.last_reward_rate = 0.0;
		self.total_times_submitted = 0;
		self.total_submit_mills = 0;
		self.last_submit_time = 0;
		self.total_mining_mills = 0;
		self.last_mine_time = 0;
	}
}