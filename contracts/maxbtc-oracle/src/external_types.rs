use cosmwasm_std::{Decimal, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct ExchangeRate {
    pub height: u64,
    pub timestamp: Timestamp,
    pub exchange_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoreQueryMsg {
    ExchangeRate {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RedemptionRateResponse {
    pub redemption_rate: Decimal,
    pub update_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DualRedemptionRateResponse {
    pub maxbtc_redemption_rate: Decimal,
    pub lst_redemption_rate: Decimal,
    pub maxbtc_update_time: u64,
    pub lst_update_time: u64,
}
