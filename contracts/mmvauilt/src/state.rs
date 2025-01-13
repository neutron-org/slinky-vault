use cosmwasm_std::{Addr, Coin, Uint128};
use cw_storage_plus::Item;
use neutron_std::types::slinky::types::v1::CurrencyPair;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DEX_WITHDRAW_REPLY_ID: u64 = 1;
pub const CRON_MODULE_ADDRESS: &str = "neutron1cd6wafvehv79pm2yxth40thpyc7dc0yrqkyk95";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TokenData {
    pub denom: String,
    pub decimals: u8,
    pub pair: CurrencyPair,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PairData {
    pub token_0: TokenData,
    pub token_1: TokenData,
    pub pair_id: String,
}

/// This structure stores the concentrated pair parameters.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Balances {
    pub token_0: Coin,
    pub token_1: Coin,
}

/// This structure stores the concentrated pair parameters.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// number of blocks until price is stale
    pub pair_data: PairData,
    pub max_blocks_old: u64,
    pub balances: Balances,
    pub base_fee: u64,
    pub base_deposit_percentage: u64,
    pub ambient_fee: u64,
    pub deposit_ambient: bool,
    pub owner: Addr,
    pub deposit_cap: Uint128,
}

// pub const PAIRDATA: Item<PairData> = Item::new("data");
pub const CONFIG: Item<Config> = Item::new("data");
