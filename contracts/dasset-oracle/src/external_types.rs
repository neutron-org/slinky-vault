use cosmwasm_std::{Decimal, Uint128, Uint64};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsgDrop {
    Config {},
    Owner {},
    ExchangeRate {},
    CurrentUnbondBatch {},
    UnbondBatch {
        batch_id: Uint128,
    },
    UnbondBatches {
        limit: Option<Uint64>,
        page_key: Option<Uint128>,
    },
    ContractState {},
    LastPuppeteerResponse {},
    TotalBonded {},
    BondProviders {},
    TotalAsyncTokens {},
    FailedBatch {},
    Pause {},
    BondHooks {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RedemptionRateResponse {
    pub redemption_rate: Decimal,
    pub update_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UnbondingPeriodResponse {
    pub unbonding_period: u64,
    pub update_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub factory_contract: String,
    pub base_denom: String,
    pub remote_denom: String,
    pub idle_min_interval: u64,
    pub unbonding_period: u64,
    pub unbonding_safe_period: u64,
    pub unbond_batch_switch_time: u64,
    pub pump_ica_address: Option<String>,
    pub transfer_channel_id: String,
    pub emergency_address: Option<String>,
    pub icq_update_delay: u64,
}
