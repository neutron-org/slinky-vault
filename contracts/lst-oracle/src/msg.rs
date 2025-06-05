use crate::state::TokenData;
use cw_ownable::cw_ownable_execute;
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
    GetRedemptionRate {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct UpdateConfig {
    pub lst_asset_denom: Option<String>,
    pub redemption_rate: Option<PrecDec>,
}

#[cw_ownable_execute]
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
    pub owner: String,
    pub lst_asset_denom: String,
    pub redemption_rate: PrecDec,
}
