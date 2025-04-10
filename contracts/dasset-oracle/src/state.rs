use cosmwasm_std::Addr;
use cw_storage_plus::Item;
use neutron_std::types::neutron::util::precdec::PrecDec;
use neutron_std::types::slinky::types::v1::CurrencyPair;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TokenData {
    pub denom: String,
    pub decimals: u8,
    pub pair: CurrencyPair,
    pub max_blocks_old: u64,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct CombinedPriceResponse {
    pub token_0_price: PrecDec,
    pub token_1_price: PrecDec,
    pub price_0_to_1: PrecDec,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Config {
    pub core_contract: Addr,
    pub d_asset_denom: String,
    pub staking_rewards: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");
