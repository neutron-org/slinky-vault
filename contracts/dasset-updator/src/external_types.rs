use cosmwasm_std::{Decimal, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct ExchangeRate {
    pub height: u64,
    pub timestamp: Timestamp,
    pub exchange_rate: Decimal,
}

#[cw_serde]
pub struct DropInstanceApy {
    pub instance: String,
    pub start_exchange_rate: ExchangeRate,
    pub end_exchange_rate: ExchangeRate,
    pub apy: Decimal,
}

#[cw_serde]
pub struct AllApyResponse {
    pub apys: Vec<Decimal>,
}

#[cw_serde]
pub struct CalculatedFeeTiers {
    pub denom: String,
    pub apy: Decimal,
    pub base_fee: u64,
    pub oracle_skew: i32,
    pub fee_tiers: Vec<(u64, u64)>, // (fee, percentage) pairs
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema, QueryResponses)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    #[returns(DropInstanceApy)]
    GetApy {
        instance: String,
        time_span_hours: u64,
    },
}