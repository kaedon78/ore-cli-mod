#[cfg(feature = "ore")]
pub const TOKEN_NAME: &str = "ORE";
#[cfg(feature = "orz")]
pub const TOKEN_NAME: &str = "ORZ";
pub const CU_LIMIT_CLAIM: u32 = 1_000 + (10_000 * 5);
pub const CU_LIMIT_RESET: u32 = 12_200;
pub const CU_LIMIT_MINE: u32 = 500 + (3250 * 5);  //rough ix cost based on what i see in the logs * number of wallets
pub const RESET_ODDS: u64 = 20;

pub const GPU_SYNC_FILE: &str = "sync_file.txt";
pub const TARGET_RATE_MULT: f64 = 0.875;
pub const NONCE_RENT: u64 = 1_447_680;

pub const RPC_RETRIES: usize = 0;
pub const GATEWAY_RETRIES: usize = 150;
pub const CONFIRM_RETRIES: usize = 1;
pub const CONFIRM_DELAY: u64 = 0;
pub const GATEWAY_DELAY: u64 = 300;