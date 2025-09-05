use crate::state::TokenData;

use cw_ownable::cw_ownable_execute;
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
    GetLstAssetDenom {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct UpdateConfig {
    pub core_contract: Option<String>,
    pub d_asset_denom: Option<String>,
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
    pub core_contract: String,
    pub owner: String,
    pub d_asset_denom: String,
}
