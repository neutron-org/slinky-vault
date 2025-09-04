use cosmwasm_std::Addr;
use crate::msg::AssetData;
use cw_storage_plus::Item;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Config {
    pub assets: Vec<AssetData>,
    pub apy_contract: Addr,
    pub whitelist: Vec<Addr>,
}

pub const CONFIG: Item<Config> = Item::new("config");
