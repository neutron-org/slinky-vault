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
