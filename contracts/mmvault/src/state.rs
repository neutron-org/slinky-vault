use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::Item;
use neutron_std::types::slinky::types::v1::CurrencyPair;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const CREATE_TOKEN_REPLY_ID: u64 = 1;
pub const WITHDRAW_REPLY_ID: u64 = 2;
pub const DEX_DEPOSIT_REPLY_ID: u64 = 3;
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

impl std::fmt::Display for FeeTierConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.fee_tiers)
    }
}

/// This structure stores the concentrated pair parameters.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// token and denom information
    pub pair_data: PairData,
    /// the denom of the contract's LP token
    pub lp_denom: String,
    /// total number of LP shares in existance
    pub total_shares: Uint128,
    /// list of addresses that can update the config and run restricted functions like dex_withdrawal and dex_deposit.
    pub whitelist: Vec<Addr>,
    /// maximum amount of dollar value that can be deposited into the contract
    pub deposit_cap: Uint128,
    /// location and weights of Deposits to be created
    pub fee_tier_config: FeeTierConfig,
    /// number of blocks until the contract is deemed stale.
    /// Once stale, the contract will be paused for 1 block before being allowed to execute again.
    pub timestamp_stale: u64,
    /// last block that action was executed to prevent staleness.
    pub last_executed: u64,
    /// The block when the contract was last paused due to stalenesss.
    pub pause_block: u64,
    /// whether the contract is paused. Paused contract cannot perform deposit functionalities.
    pub paused: bool,
    /// the oracle contract address. This contract will be used to get the price of the tokens.
    pub oracle_contract: Addr,
    /// whether to skew the AMM Deposits. If >0 , the AMM Deposit index will be skewed
    /// making the over-supplied asset cheeper AND the under-supplied asset more expensive.
    pub skew: i32,
    /// the imbalance Factor indicated the rebalancing aggresiveness.
    pub imbalance: u32,
    /// General flat skew to add to the final deposit index of the vault
    pub oracle_price_skew: i32,
    /// the dynamic spread factor defines how quickly the undersupplied asset becomes more expensive
    pub dynamic_spread_factor: i32,
    /// the dynamic spread cap defines the maximum amount the undersupplied asset can be marked up in basis points.
    pub dynamic_spread_cap: i32,

}

pub const CONFIG: Item<Config> = Item::new("data");
