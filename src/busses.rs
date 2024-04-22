#[cfg(feature = "ore")]
use ore::{state::Bus, utils::AccountDeserialize, BUS_ADDRESSES, TOKEN_DECIMALS};
#[cfg(feature = "orz")]
use orz::{state::Bus, utils::AccountDeserialize, BUS_ADDRESSES, TOKEN_DECIMALS};

use solana_client::client_error::Result;
use crate::{constants::TOKEN_NAME, Miner};

impl Miner {
    pub async fn busses(&self) {
        let client = self.rpc_client.clone();
        for address in BUS_ADDRESSES.iter() {
            self.stats.borrow_mut().add_api_call("getaccountinfo");
            let data = client.get_account_data(address).await.unwrap();
            match Bus::try_from_bytes(&data) {
                Ok(bus) => {
                    let rewards = (bus.rewards as f64) / 10f64.powf(TOKEN_DECIMALS as f64);
                    println!("Bus {}: {:} {}", bus.id, rewards, TOKEN_NAME);
                }
                Err(_) => {}
            }
        }
    }

    pub async fn get_bus(&self, id: usize) -> Result<Bus> {
        let client = self.rpc_client.clone();
        //println!("Calling getaccount for bus");
        self.stats.borrow_mut().add_api_call("getaccountinfo");
        let data = client.get_account_data(&BUS_ADDRESSES[id]).await?;
        Ok(*Bus::try_from_bytes(&data).unwrap())
    }
}
