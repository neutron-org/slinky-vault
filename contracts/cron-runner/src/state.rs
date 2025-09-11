use cosmwasm_std::Addr;
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Config {
    /// List of vault contract addresses to rebalance
    pub vault_addresses: Vec<Addr>,
    /// Address authorized to call the main rebalancing function (typically the cron module)
    pub cron_address: Addr,
    /// Address authorized to update configuration and manage the contract
    pub admin_address: Addr,
}

pub const CONFIG: Item<Config> = Item::new("config");
