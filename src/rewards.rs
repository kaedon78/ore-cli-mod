use std::str::FromStr;
use solana_sdk::{
    signature::Signer,
    signer::keypair::Keypair
};
use solana_program::pubkey::Pubkey;

use crate::{utils::get_proof, Miner};

impl Miner {

    pub async fn all_rewards(&self) {
        for wallet in self.wallets.iter() {
            self.rewards(wallet).await
        }
    }

    pub async fn rewards(&self, wallet:&Keypair) {
        let proof = get_proof(&self.rpc_client, wallet.pubkey()).await;
        let amount = (proof.claimable_rewards as f64) / 10f64.powf(ore::TOKEN_DECIMALS as f64);
        println!("{:} ORE", amount);
    }
}
