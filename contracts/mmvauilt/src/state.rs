use cosmwasm_std::{Addr, Int64};
use cw_storage_plus::Item;
use neutron_sdk::bindings::oracle::types::CurrencyPair;
use crate::{
    error::{ContractError, ContractResult},
};

// use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Coin, Response};
use neutron_sdk::bindings::marketmap::query::{MarketMapQuery, MarketMapResponse, MarketResponse};
use neutron_sdk::bindings::{msg::NeutronMsg, query::NeutronQuery};


use neutron_sdk::bindings::oracle::query::{
    GetAllCurrencyPairsResponse, GetPriceResponse, GetPricesResponse, OracleQuery,
};
use cosmwasm_std::Uint64;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct TokenData {
    pub denom: String,
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
    pub token_1: Coin
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
    pub owner: Addr,
}

// pub const PAIRDATA: Item<PairData> = Item::new("data");
pub const CONFIG: Item<Config> = Item::new("data");
