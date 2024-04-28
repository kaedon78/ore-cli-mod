mod balance;
mod busses;
mod claim;
mod constants;
#[cfg(feature = "admin")]
mod initialize;
mod mine;
//mod nonce_manager;
mod register;
mod rewards;
mod stats;
mod treasury;
mod transaction;
#[cfg(feature = "admin")]
mod update_admin;
#[cfg(feature = "admin")]
mod update_difficulty;
mod utils;

use std::{
    cell::RefCell, sync::Arc, //default,
};

use clap::{command, Parser, Subcommand};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{read_keypair_file, Keypair},
};
use sysinfo::System;
use stats::MinerStats;

struct Miner {
    pub priority_fee: u64,
    pub rpc_client: Arc<RpcClient>,
    pub use_gpu: bool,
    pub wallets: Vec<Keypair>,
    pub keypair_fee: Option<Keypair>,
    pub threads: u64,
    pub stats: RefCell<MinerStats>
}

#[derive(Parser, Debug)]
#[command(about, version)]
struct Args {
    #[arg(
        long,
        value_name = "NETWORK_URL",
        help = "Network address of your RPC provider",
        global = true
    )]
    rpc: Option<String>,

    #[clap(
        global = true,
        short = 'C',
        long = "config",
        id = "PATH",
        help = "Filepath to config file."
    )]
    pub config_file: Option<String>,

    #[arg(
        long,
        value_name = "KEYPAIR_FILEPATH1",
        help = "Filepath to keypair 1 to use",
        global = true
    )]
    keypair1: Option<String>,

    #[arg(
        long,
        value_name = "KEYPAIR_FILEPATH2",
        help = "Filepath to keypair 2 to use",
        global = true
    )]
    keypair2: Option<String>,

    #[arg(
        long,
        value_name = "KEYPAIR_FILEPATH3",
        help = "Filepath to keypair 3 to use",
        global = true
    )]
    keypair3: Option<String>,
    
    #[arg(
        long,
        value_name = "KEYPAIR_FILEPATH4",
        help = "Filepath to keypair 4 to use",
        global = true
    )]
    keypair4: Option<String>,

    #[arg(
        long,
        value_name = "KEYPAIR_FILEPATH5",
        help = "Filepath to keypair 5 to use",
        global = true
    )]
    keypair5: Option<String>,

    #[arg(
        long,
        value_name = "MICROLAMPORTS",
        help = "Number of microlamports to pay as priority fee per transaction",
        default_value = "0",
        global = true
    )]
    priority_fee: u64,
    
    #[arg(
        long,
        value_name = "USE_GPU",
        help = "Use GPU instead of CPU",
        default_value = "0",
        global = true
    )]
    use_gpu: u64,

    #[arg(
        long,
        value_name = "THREADS",
        help = "Thread count for CPU mining only",
        default_value = "0",
        global = true
    )]
    threads: u64,    

    #[arg(
        long,
        value_name = "KEYPAIR_FEE_FILEPATH",
        help = "Filepath to fee payer keypair to use",
        global = true
    )]
    keypair_fee: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Fetch the Ore balance of an account")]
    Balance(BalanceArgs),

    #[command(about = "Fetch the distributable rewards of the busses")]
    Busses(BussesArgs),

    #[command(about = "Mine Ore using local compute")]
    Mine(MineArgs),

    #[command(about = "Claim available mining rewards")]
    Claim(ClaimArgs),

    #[command(about = "Fetch your balance of unclaimed mining rewards")]
    Rewards(RewardsArgs),

    #[command(about = "Fetch the treasury account and balance")]
    Treasury(TreasuryArgs),

    #[cfg(feature = "admin")]
    #[command(about = "Initialize the program")]
    Initialize(InitializeArgs),

    #[cfg(feature = "admin")]
    #[command(about = "Update the program admin authority")]
    UpdateAdmin(UpdateAdminArgs),

    #[cfg(feature = "admin")]
    #[command(about = "Update the mining difficulty")]
    UpdateDifficulty(UpdateDifficultyArgs),
}

#[derive(Parser, Debug)]
struct BalanceArgs {
    #[arg(
        // long,
        value_name = "ADDRESS",
        help = "The address of the account to fetch the balance of"
    )]
    pub address: Option<String>,
}

#[derive(Parser, Debug)]
struct BussesArgs {}

#[derive(Parser, Debug)]
struct RewardsArgs {
    #[arg(
        // long,
        value_name = "ADDRESS",
        help = "The address of the account to fetch the rewards balance of"
    )]
    pub address: Option<String>,
}

#[derive(Parser, Debug)]
struct MineArgs {}

#[derive(Parser, Debug)]
struct TreasuryArgs {}

#[derive(Parser, Debug)]
struct ClaimArgs {
    #[arg(
        // long,
        value_name = "AMOUNT",
        help = "The amount of rewards to claim. Defaults to max."
    )]
    amount: Option<f64>,
}

#[cfg(feature = "admin")]
#[derive(Parser, Debug)]
struct InitializeArgs {}

#[cfg(feature = "admin")]
#[derive(Parser, Debug)]
struct UpdateAdminArgs {
    new_admin: String,
}

#[cfg(feature = "admin")]
#[derive(Parser, Debug)]
struct UpdateDifficultyArgs {}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Load the config file from custom path, the default path, or use default config values
    let cli_config = if let Some(config_file) = &args.config_file {
        solana_cli_config::Config::load(config_file).unwrap_or_else(|_| {
            eprintln!("error: Could not find config file `{}`", config_file);
            std::process::exit(1);
        })
    } else if let Some(config_file) = &*solana_cli_config::CONFIG_FILE {
        solana_cli_config::Config::load(config_file).unwrap_or_default()
    } else {
        solana_cli_config::Config::default()
    };

    // Initialize miner.
    let cluster = args.rpc.unwrap_or(cli_config.json_rpc_url);
    //println!("URL {}", cluster);
    let mut wallets = vec![];
    let default_keypair1 = args.keypair1.unwrap_or(cli_config.keypair_path.clone());
    wallets.push(read_wallet(default_keypair1));
    
    let default_keypair2 = args.keypair2.unwrap_or("".to_string());
    if default_keypair2 != "" {wallets.push(read_wallet(default_keypair2));}
    
    let default_keypair3 = args.keypair3.unwrap_or("".to_string());
    if default_keypair3 != "" {wallets.push(read_wallet(default_keypair3));}
    
    let default_keypair4 = args.keypair4.unwrap_or("".to_string());
    if default_keypair4 != "" {wallets.push(read_wallet(default_keypair4));}
    
    let default_keypair5 = args.keypair5.unwrap_or("".to_string());
    if default_keypair5 != "" {wallets.push(read_wallet(default_keypair5));}

    let keypair_fee: Option<Keypair> = if let Some(keypair_fee) = args.keypair_fee {
        Some(read_wallet(keypair_fee))
    }   
    else {
        None
    };

    let rpc_client = RpcClient::new_with_commitment(cluster, CommitmentConfig::confirmed());
    let use_gpu = args.use_gpu != 0;
    let mut threads = args.threads;
    
    if !use_gpu {
        if threads == 0 {
            println!("Initializing thread count...");
            let mut system = System::new_all();
            system.refresh_all();
            println!("\tCPU: {} {}, {} Cores, {} Mhz", system.cpus()[0].brand(), system.cpus()[0].name(), system.cpus().len(), system.cpus()[0].frequency());
            threads += system.cpus().len() as u64;
        }
    }
    let stats = RefCell::new(MinerStats::new());
    
    let miner = Arc::new(Miner::new(
        Arc::new(rpc_client),
        args.priority_fee,
        use_gpu,
        wallets,
        keypair_fee,
        threads,
        stats
    ));

    // Execute user command.
    match args.command {
        Commands::Balance(_args) => {
            miner.all_balances().await;
        }
        Commands::Busses(_) => {
            miner.busses().await;
        }
        Commands::Rewards(_args) => {
            miner.all_rewards().await;
        }
        Commands::Treasury(_) => {
            miner.treasury().await;
        }
        Commands::Mine(_) => {
            miner.mine().await;
        }
        Commands::Claim(args) => {
            miner.claim_all(args.amount).await;
        }
        #[cfg(feature = "admin")]
        Commands::Initialize(_) => {
            miner.initialize().await;
        }
        #[cfg(feature = "admin")]
        Commands::UpdateAdmin(args) => {
            miner.update_admin(args.new_admin).await;
        }
        #[cfg(feature = "admin")]
        Commands::UpdateDifficulty(_) => {
            miner.update_difficulty().await;
        }
    }
}

pub fn read_wallet(keypair_filepath: String) -> Keypair {
    if keypair_filepath != "" {
        read_keypair_file(keypair_filepath).unwrap()
    }
    else {
        panic!("No keypair provided")
    }
}

impl Miner {
    pub fn new(
        rpc_client: Arc<RpcClient>, 
        priority_fee: u64, 
        use_gpu: bool,
        wallets: Vec<Keypair>,
        keypair_fee: Option<Keypair>,
        threads: u64,
        stats: RefCell<MinerStats>,
    ) -> Self {
        Self {
            rpc_client,
            priority_fee,
            use_gpu,
            wallets,
            keypair_fee,
            threads,
            stats,
        }
    }
}