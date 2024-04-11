use solana_sdk::{
    signature::Signer,
    signer::keypair::Keypair
};
use crate::Miner;

impl Miner {
    pub async fn balance_by_number(&self, keypair_number: u64) {
        self.balance(&self.signer_by_number(keypair_number)).await
    }

    pub async fn balance(&self, signer: &Keypair) {
        let address = signer.pubkey();
        let client = self.rpc_client.clone();
        let token_account_address = spl_associated_token_account::get_associated_token_address(
            &address,
            &ore::MINT_ADDRESS,
        );
        match client.get_token_account(&token_account_address).await {
            Ok(token_account) => {
                if let Some(token_account) = token_account {
                    println!("{:} ORE", token_account.token_amount.ui_amount_string);
                } else {
                    println!("Account not found");
                }
            }
            Err(err) => {
                println!("{:?}", err);
            }
        }
    }
}
