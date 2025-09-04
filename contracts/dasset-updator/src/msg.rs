
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_schema::QueryResponses;
use crate::state::Config;
use crate::external_types::{AllApyResponse, CalculatedFeeTiers};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema, QueryResponses)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    #[returns(Config)]
    GetConfig {},
    #[returns(AllApyResponse)]
    GetAllApy {},
    #[returns(Vec<CalculatedFeeTiers>)]
    GetFeeTiers {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct UpdateConfig {
    pub new_assets: Option<Vec<AssetData>>,
    pub new_apy_contract: Option<String>,
    pub new_whitelist: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig {
        new_config: UpdateConfig,
    },
    RunVaultUpdate {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct AssetData {
    pub denom: String,// denom of the dasset
    pub core_contract: String, // core contract address of the dasset
    pub unbonding_period: u64, // unbonding period of the dasset in days
    pub fee_spacings: Vec<u64>, // fee spacings to add on-top of the calculated base fee
    pub percentages: Vec<u64>, // percentages of the dasset on each fee spacing
    pub vault_address: String, // vault address for the dasset
    pub query_period_hours: u64, // query period in hours for the dasset apy contract
    pub fee_dempening_amount: u64, // amount to dempen the fee calculation by in pps
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct InstantiateMsg {
    pub assets: Vec<AssetData>,
    pub apy_contract: String,
    pub whitelist: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct UpdateVaultConfig {
    // placeholder for future vault configuration updates
}
