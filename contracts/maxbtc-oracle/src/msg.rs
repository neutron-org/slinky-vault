use crate::state::TokenData;
use neutron_std::types::neutron::util::precdec::PrecDec;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    GetPrices {
        token_a: TokenData,
        token_b: TokenData,
    },
    GetRedemptionRates {},
    GetMaxBtcRedemptionRate {},
    GetLstRedemptionRate {},
    GetMaxBtcDenom {},
    GetLstDenom {},
    GetOwners {},
    IsOwner { address: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct UpdateConfig {
    pub owners: Option<Vec<String>>,
    pub maxbtc_core_contract: Option<String>,
    pub maxbtc_denom: Option<String>,
    pub lst_denom: Option<String>,
    pub lst_redemption_rate: Option<PrecDec>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig { new_config: UpdateConfig },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub initial_owners: Vec<String>,
    pub maxbtc_core_contract: String,
    pub maxbtc_denom: String,
    pub lst_denom: String,
    pub lst_redemption_rate: PrecDec,
}
