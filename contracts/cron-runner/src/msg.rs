use crate::state::Config;
use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;

#[cw_serde]
pub struct InstantiateMsg {
    /// List of vault contract addresses to rebalance
    pub vault_addresses: Vec<String>,
    /// Address authorized to call the main rebalancing function (typically the cron module)
    pub cron_address: String,
    /// Address authorized to update configuration and manage the contract
    pub admin_address: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Main function called by the cron module to rebalance all vaults
    /// This calls dex_withdrawal followed by dex_deposit on each vault
    RunRebalancing {},
    /// Update the contract configuration (admin-only)
    UpdateConfig { new_config: ConfigUpdate },
}

#[cw_serde]
pub struct ConfigUpdate {
    /// New list of vault addresses (replaces current list)
    pub vault_addresses: Option<Vec<String>>,
    /// New cron address
    pub cron_address: Option<String>,
    /// New admin address
    pub admin_address: Option<String>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Get the current configuration
    #[returns(Config)]
    GetConfig {},
    /// Get the list of vault addresses
    #[returns(VaultListResponse)]
    GetVaultList {},
}

#[cw_serde]
pub struct VaultListResponse {
    pub vault_addresses: Vec<Addr>,
}

#[cw_serde]
pub struct MigrateMsg {
    /// Optional new configuration during migration
    pub new_config: Option<Config>,
}
