#[cfg(feature = "ore")]
use ore::{self, TOKEN_DECIMALS};
#[cfg(feature = "orz")]
use orz::{self, TOKEN_DECIMALS};

use solana_sdk::{
    signature::Signer,
    signer::keypair::Keypair
};

use crate::{constants::TOKEN_NAME, utils::get_proof, Miner};

impl Miner {

    pub async fn all_rewards(&self) {
        for wallet in self.wallets.iter() {
            self.rewards(wallet).await
        }
    }

    pub async fn rewards(&self, wallet:&Keypair) {
        self.stats.borrow_mut().add_api_call("getaccountinfo");    
        let proof = get_proof(&self.rpc_client, wallet.pubkey()).await;
        let amount = (proof.claimable_rewards as f64) / 10f64.powf(TOKEN_DECIMALS as f64);
        println!("{:} {}", amount, TOKEN_NAME);
    }
}
