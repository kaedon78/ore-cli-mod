mod balance;
mod busses;
mod claim;
mod cu_limits;
#[cfg(feature = "admin")]
mod initialize;
mod mine;
mod register;
mod rewards;
mod send_and_confirm;
mod treasury;
#[cfg(feature = "admin")]
mod update_admin;
#[cfg(feature = "admin")]
mod update_difficulty;
mod utils;

use std::sync::Arc;

use clap::{command, Parser, Subcommand};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{read_keypair_file, Keypair},
};

const WALLETS: u64 = 5;

struct Miner {
    pub keypair_filepath1: Option<String>,
    pub keypair_filepath2: Option<String>,
    pub keypair_filepath3: Option<String>,
    pub keypair_filepath4: Option<String>,
    pub keypair_filepath5: Option<String>,
    pub priority_fee: u64,
    pub rpc_client: Arc<RpcClient>,
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
struct MineArgs {
    #[arg(
        long,
        short,
        value_name = "THREAD_COUNT",
        help = "The number of threads to dedicate to mining",
        default_value = "1"
    )]
    threads: u64,
}

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

    #[arg(
        // long,
        value_name = "TOKEN_ACCOUNT_ADDRESS",
        help = "Token account to receive mining rewards."
    )]
    beneficiary: Option<String>,
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
    println!("URL {}", cluster);
    let default_keypair1 = args.keypair1.unwrap_or(cli_config.keypair_path.clone());
    let default_keypair2 = args.keypair2.unwrap_or("".to_string());
    let default_keypair3 = args.keypair3.unwrap_or("".to_string());
    let default_keypair4 = args.keypair4.unwrap_or("".to_string());
    let default_keypair5 = args.keypair5.unwrap_or("".to_string());

    let rpc_client = RpcClient::new_with_commitment(cluster, CommitmentConfig::confirmed());

    let miner = Arc::new(Miner::new(
        Arc::new(rpc_client),
        args.priority_fee,
        Some(default_keypair1),
        Some(default_keypair2),
        Some(default_keypair3),
        Some(default_keypair4),
        Some(default_keypair5),
    ));

    // Execute user command.
    match args.command {
        Commands::Balance(_args) => {
            for wallet in 1..WALLETS+1 {
                miner.balance_by_number(wallet).await;
            }
        }
        Commands::Busses(_) => {
            miner.busses().await;
        }
        Commands::Rewards(_args) => {
            for wallet in 1..WALLETS+1 {    
                miner.rewards_by_number(wallet).await;
            }
        }
        Commands::Treasury(_) => {
            miner.treasury().await;
        }
        Commands::Mine(args) => {
            miner.mine(args.threads).await;
        }
        Commands::Claim(args) => {
            miner.claim(args.beneficiary.clone(), args.amount).await;
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

impl Miner {
    pub fn new(
        rpc_client: Arc<RpcClient>, 
        priority_fee: u64, 
        keypair_filepath1: Option<String>, 
        keypair_filepath2: Option<String>, 
        keypair_filepath3: Option<String>,
        keypair_filepath4: Option<String>,
        keypair_filepath5: Option<String>,
    ) -> Self {
        Self {
            rpc_client,
            keypair_filepath1,
            keypair_filepath2,
            keypair_filepath3,
            keypair_filepath4,
            keypair_filepath5,
            priority_fee,
        }
    }

    pub fn signer(&self) -> Keypair {
        self.signer_by_number(1)
    }

    pub fn signer_by_number(&self, keypair_number: u64) -> Keypair {
        let mut keypair_filepath = &self.keypair_filepath1;
        if keypair_number == 2 {
            keypair_filepath = &self.keypair_filepath2;
        }
        if keypair_number == 3 {
            keypair_filepath = &self.keypair_filepath3;
        }
        if keypair_number == 4 {
            keypair_filepath = &self.keypair_filepath4;
        }
        if keypair_number == 5 {
            keypair_filepath = &self.keypair_filepath5;
        }

        match keypair_filepath.clone() {
            Some(filepath) => read_keypair_file(filepath).unwrap(),
            None => panic!("No keypair provided"),
        }
    }
}
