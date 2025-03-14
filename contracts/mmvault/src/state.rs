use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::Item;
use neutron_std::types::slinky::types::v1::CurrencyPair;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CREATE_TOKEN_REPLY_ID: u64 = 1;
pub const WITHDRAW_REPLY_ID: u64 = 2;
pub const DEX_DEPOSIT_REPLY_ID_1: u64 = 3;
pub const DEX_DEPOSIT_REPLY_ID_2: u64 = 4;
pub const SHARES_MULTIPLIER: u64 = 1000000000;


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TokenData {
    pub denom: String,
    pub decimals: u8,
    pub pair: CurrencyPair,
    pub max_blocks_old: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct PairData {
    pub token_0: TokenData,
    pub token_1: TokenData,
    pub pair_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct FeeTier {
    pub fee: u64,
    pub percentage: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct FeeTierConfig {
    pub fee_tiers: Vec<FeeTier>,
}

/// This structure stores the concentrated pair parameters.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// number of blocks until price is stale
    pub pair_data: PairData,
    pub lp_denom: String,
    pub total_shares: Uint128,
    pub whitelist: Vec<Addr>,
    pub deposit_cap: Uint128,
    pub fee_tier_config: FeeTierConfig,
    pub timestamp_stale: u64,
    pub last_executed: u64,
    pub pause_block: u64,
    pub paused: bool,
    pub oracle_contract: Addr,
    pub skew: bool,
    pub imbalance: u32,
}

// pub const PAIRDATA: Item<PairData> = Item::new("data");
pub const CONFIG: Item<Config> = Item::new("data");
