use crate::error::{ContractError, ContractResult};
use crate::msg::{ConfigUpdate, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, VaultListResponse};
use crate::state::{Config, CONFIG};
use crate::utils::*;
use cosmwasm_std::{
    attr, entry_point, to_json_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response,
};
use cw2::set_contract_version;

const CONTRACT_NAME: &str = concat!("crates.io:neutron-contracts__", env!("CARGO_PKG_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

///////////////////
/// INSTANTIATE ///
///////////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    // Set contract version for migration info
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Validate input
    validate_vault_addresses(&msg.vault_addresses)?;
    validate_address(&msg.cron_address, "cron_address")?;
    validate_address(&msg.admin_address, "admin_address")?;

    // Validate and convert addresses
    let vault_addresses = msg
        .vault_addresses
        .iter()
        .map(|addr| deps.api.addr_validate(addr))
        .collect::<Result<Vec<Addr>, _>>()?;

    let cron_address = deps.api.addr_validate(&msg.cron_address)?;
    let admin_address = deps.api.addr_validate(&msg.admin_address)?;

    // Create and save config
    let config = Config {
        vault_addresses: vault_addresses.clone(),
        cron_address: cron_address.clone(),
        admin_address: admin_address.clone(),
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("vault_count", vault_addresses.len().to_string())
        .add_attributes([
            attr("cron_address", cron_address.to_string()),
            attr("admin_address", admin_address.to_string()),
        ]))
}

///////////////
/// EXECUTE ///
///////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::RunRebalancing {} => execute_run_rebalancing(deps, info),
        ExecuteMsg::UpdateConfig { new_config } => execute_update_config(deps, info, new_config),
    }
}

/// Execute the main rebalancing function - calls dex_withdrawal and dex_deposit on all vaults
fn execute_run_rebalancing(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Check authorization - either cron address or admin address can call this
    if info.sender != config.cron_address && info.sender != config.admin_address {
        return Err(ContractError::Unauthorized);
    }

    let mut messages = Vec::new();
    let mut attributes = Vec::new();

    attributes.push(attr("action", "run_rebalancing"));
    attributes.push(attr("vault_count", config.vault_addresses.len().to_string()));
    attributes.push(attr("caller", info.sender.to_string()));

    // Process each vault: first withdrawal, then deposit
    for vault_address in &config.vault_addresses {
        // Create withdrawal message
        let withdrawal_msg = create_dex_withdrawal_message(vault_address)
            .map_err(|e| ContractError::MessageCreationError {
                vault: vault_address.to_string(),
                reason: format!("withdrawal: {}", e),
            })?;

        // Create deposit message
        let deposit_msg = create_dex_deposit_message(vault_address)
            .map_err(|e| ContractError::MessageCreationError {
                vault: vault_address.to_string(),
                reason: format!("deposit: {}", e),
            })?;

        messages.push(withdrawal_msg);
        messages.push(deposit_msg);

        attributes.push(attr(
            format!("vault_{}", vault_address),
            "withdrawal_and_deposit",
        ));
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(attributes))
}

/// Update the contract configuration (admin-only)
fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_config: ConfigUpdate,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    // Check authorization - only admin can update config
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized);
    }

    let mut attrs = vec![attr("action", "update_config")];

    // Update vault addresses if provided
    if let Some(new_vault_addresses) = new_config.vault_addresses {
        validate_vault_addresses(&new_vault_addresses)?;
        
        let vault_addresses = new_vault_addresses
            .iter()
            .map(|addr| deps.api.addr_validate(addr))
            .collect::<Result<Vec<Addr>, _>>()?;

        config.vault_addresses = vault_addresses.clone();
        attrs.push(attr("vault_count", vault_addresses.len().to_string()));
        attrs.push(attr("updated_vaults", "true"));
    }

    // Update cron address if provided
    if let Some(new_cron_address) = new_config.cron_address {
        validate_address(&new_cron_address, "cron_address")?;
        let cron_address = deps.api.addr_validate(&new_cron_address)?;
        config.cron_address = cron_address.clone();
        attrs.push(attr("new_cron_address", cron_address.to_string()));
    }

    // Update admin address if provided
    if let Some(new_admin_address) = new_config.admin_address {
        validate_address(&new_admin_address, "admin_address")?;
        let admin_address = deps.api.addr_validate(&new_admin_address)?;
        config.admin_address = admin_address.clone();
        attrs.push(attr("new_admin_address", admin_address.to_string()));
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attributes(attrs))
}

/////////////
/// QUERY ///
/////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => {
            let config = CONFIG.load(deps.storage)?;
            to_json_binary(&config).map_err(|_| ContractError::SerializationError)
        }
        QueryMsg::GetVaultList {} => {
            let config = CONFIG.load(deps.storage)?;
            let response = VaultListResponse {
                vault_addresses: config.vault_addresses,
            };
            to_json_binary(&response).map_err(|_| ContractError::SerializationError)
        }
    }
}

///////////////
/// MIGRATE ///
///////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    // Update contract version
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // If new config is provided during migration, update it
    if let Some(new_config) = msg.new_config {
        CONFIG.save(deps.storage, &new_config)?;
    }

    Ok(Response::new()
        .add_attribute("action", "migrate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{message_info, mock_dependencies, mock_env};
    use cosmwasm_std::{from_json, CosmosMsg, WasmMsg};

    #[test]
    fn test_instantiate_success() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = message_info(&deps.api.addr_make("creator"), &[]);

        let vault1 = deps.api.addr_make("vault1").to_string();
        let vault2 = deps.api.addr_make("vault2").to_string();
        let cron_addr = deps.api.addr_make("cron").to_string();
        let admin_addr = deps.api.addr_make("admin").to_string();
        
        let msg = InstantiateMsg {
            vault_addresses: vec![vault1.clone(), vault2.clone()],
            cron_address: cron_addr.clone(),
            admin_address: admin_addr.clone(),
        };

        let result = instantiate(deps.as_mut(), env, info, msg);
        assert!(result.is_ok());

        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.vault_addresses.len(), 2);
        assert_eq!(config.cron_address, deps.api.addr_make("cron"));
        assert_eq!(config.admin_address, deps.api.addr_make("admin"));
    }

    #[test]
    fn test_instantiate_empty_vaults() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = message_info(&deps.api.addr_make("creator"), &[]);

        let msg = InstantiateMsg {
            vault_addresses: vec![],
            cron_address: deps.api.addr_make("cron").to_string(),
            admin_address: deps.api.addr_make("admin").to_string(),
        };

        let result = instantiate(deps.as_mut(), env, info, msg);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ContractError::NoVaultsConfigured => {},
            _ => panic!("Expected NoVaultsConfigured error"),
        }
    }

    #[test]
    fn test_instantiate_duplicate_vaults() {
        let mut deps = mock_dependencies();
        let env = mock_env();
        let info = message_info(&deps.api.addr_make("creator"), &[]);

        let vault1 = deps.api.addr_make("vault1").to_string();
        let msg = InstantiateMsg {
            vault_addresses: vec![
                vault1.clone(),
                vault1.clone(), // duplicate
            ],
            cron_address: deps.api.addr_make("cron").to_string(),
            admin_address: deps.api.addr_make("admin").to_string(),
        };

        let result = instantiate(deps.as_mut(), env, info, msg);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ContractError::DuplicateVaultAddress { .. } => {},
            _ => panic!("Expected DuplicateVaultAddress error"),
        }
    }

    #[test]
    fn test_run_rebalancing_success() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        // Setup config
        let vault1 = deps.api.addr_make("vault1");
        let vault2 = deps.api.addr_make("vault2");
        let cron_addr = deps.api.addr_make("cron");
        let admin_addr = deps.api.addr_make("admin");
        
        let config = Config {
            vault_addresses: vec![vault1.clone(), vault2.clone()],
            cron_address: cron_addr.clone(),
            admin_address: admin_addr.clone(),
        };
        CONFIG.save(&mut deps.storage, &config).unwrap();

        // Call from cron address
        let info = message_info(&cron_addr, &[]);
        let msg = ExecuteMsg::RunRebalancing {};

        let result = execute(deps.as_mut(), env, info, msg);
        assert!(result.is_ok());

        let response = result.unwrap();
        // Should have 4 messages: 2 withdrawals + 2 deposits
        assert_eq!(response.messages.len(), 4);

        // Check that messages are in correct order (withdrawal then deposit for each vault)
        let messages: Vec<CosmosMsg> = response.messages.into_iter().map(|msg| msg.msg).collect();
        
        match &messages[0] {
            CosmosMsg::Wasm(WasmMsg::Execute { contract_addr, .. }) => {
                assert_eq!(contract_addr, &vault1.to_string());
            }
            _ => panic!("Expected WasmMsg::Execute"),
        }
        
        match &messages[1] {
            CosmosMsg::Wasm(WasmMsg::Execute { contract_addr, .. }) => {
                assert_eq!(contract_addr, &vault1.to_string());
            }
            _ => panic!("Expected WasmMsg::Execute"),
        }

        match &messages[2] {
            CosmosMsg::Wasm(WasmMsg::Execute { contract_addr, .. }) => {
                assert_eq!(contract_addr, &vault2.to_string());
            }
            _ => panic!("Expected WasmMsg::Execute"),
        }
        
        match &messages[3] {
            CosmosMsg::Wasm(WasmMsg::Execute { contract_addr, .. }) => {
                assert_eq!(contract_addr, &vault2.to_string());
            }
            _ => panic!("Expected WasmMsg::Execute"),
        }
    }

    #[test]
    fn test_run_rebalancing_admin_success() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        // Setup config
        let vault1 = deps.api.addr_make("vault1");
        let cron_addr = deps.api.addr_make("cron");
        let admin_addr = deps.api.addr_make("admin");
        
        let config = Config {
            vault_addresses: vec![vault1.clone()],
            cron_address: cron_addr.clone(),
            admin_address: admin_addr.clone(),
        };
        CONFIG.save(&mut deps.storage, &config).unwrap();

        // Call from admin address
        let info = message_info(&admin_addr, &[]);
        let msg = ExecuteMsg::RunRebalancing {};

        let result = execute(deps.as_mut(), env, info, msg);
        assert!(result.is_ok());

        let response = result.unwrap();
        // Should have 2 messages: 1 withdrawal + 1 deposit
        assert_eq!(response.messages.len(), 2);
    }

    #[test]
    fn test_run_rebalancing_unauthorized() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        // Setup config
        let config = Config {
            vault_addresses: vec![deps.api.addr_make("vault1")],
            cron_address: deps.api.addr_make("cron"),
            admin_address: deps.api.addr_make("admin"),
        };
        CONFIG.save(&mut deps.storage, &config).unwrap();

        // Call from unauthorized address
        let info = message_info(&deps.api.addr_make("unauthorized"), &[]);
        let msg = ExecuteMsg::RunRebalancing {};

        let result = execute(deps.as_mut(), env, info, msg);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ContractError::Unauthorized => {},
            _ => panic!("Expected Unauthorized error"),
        }
    }

    #[test]
    fn test_update_config_success() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        // Setup config
        let vault1 = deps.api.addr_make("vault1");
        let cron_addr = deps.api.addr_make("cron");
        let admin_addr = deps.api.addr_make("admin");
        
        let config = Config {
            vault_addresses: vec![vault1.clone()],
            cron_address: cron_addr.clone(),
            admin_address: admin_addr.clone(),
        };
        CONFIG.save(&mut deps.storage, &config).unwrap();

        // Update config from admin address
        let vault3 = deps.api.addr_make("vault3");
        let new_cron = deps.api.addr_make("newcron");
        
        let info = message_info(&admin_addr, &[]);
        let msg = ExecuteMsg::UpdateConfig {
            new_config: ConfigUpdate {
                vault_addresses: Some(vec![
                    vault1.to_string(),
                    vault3.to_string(),
                ]),
                cron_address: Some(new_cron.to_string()),
                admin_address: None,
            },
        };

        let result = execute(deps.as_mut(), env, info, msg);
        assert!(result.is_ok());

        // Verify config was updated
        let updated_config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(updated_config.vault_addresses.len(), 2);
        assert_eq!(updated_config.vault_addresses[1], vault3);
        assert_eq!(updated_config.cron_address, new_cron);
        assert_eq!(updated_config.admin_address, admin_addr); // unchanged
    }

    #[test]
    fn test_update_config_unauthorized() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        // Setup config
        let config = Config {
            vault_addresses: vec![deps.api.addr_make("vault1")],
            cron_address: deps.api.addr_make("cron"),
            admin_address: deps.api.addr_make("admin"),
        };
        CONFIG.save(&mut deps.storage, &config).unwrap();

        // Try to update from unauthorized address
        let info = message_info(&deps.api.addr_make("unauthorized"), &[]);
        let msg = ExecuteMsg::UpdateConfig {
            new_config: ConfigUpdate {
                vault_addresses: None,
                cron_address: None,
                admin_address: None,
            },
        };

        let result = execute(deps.as_mut(), env, info, msg);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ContractError::Unauthorized => {},
            _ => panic!("Expected Unauthorized error"),
        }
    }

    #[test]
    fn test_query_config() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        // Setup config
        let vault1 = deps.api.addr_make("vault1");
        let vault2 = deps.api.addr_make("vault2");
        let cron_addr = deps.api.addr_make("cron");
        let admin_addr = deps.api.addr_make("admin");
        
        let config = Config {
            vault_addresses: vec![vault1.clone(), vault2.clone()],
            cron_address: cron_addr.clone(),
            admin_address: admin_addr.clone(),
        };
        CONFIG.save(&mut deps.storage, &config).unwrap();

        let msg = QueryMsg::GetConfig {};
        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let response: Config = from_json(&result.unwrap()).unwrap();
        assert_eq!(response.vault_addresses.len(), 2);
        assert_eq!(response.cron_address, cron_addr);
        assert_eq!(response.admin_address, admin_addr);
    }

    #[test]
    fn test_query_vault_list() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        // Setup config
        let vault1 = deps.api.addr_make("vault1");
        let vault2 = deps.api.addr_make("vault2");
        
        let config = Config {
            vault_addresses: vec![vault1.clone(), vault2.clone()],
            cron_address: deps.api.addr_make("cron"),
            admin_address: deps.api.addr_make("admin"),
        };
        CONFIG.save(&mut deps.storage, &config).unwrap();

        let msg = QueryMsg::GetVaultList {};
        let result = query(deps.as_ref(), env, msg);
        assert!(result.is_ok());

        let response: VaultListResponse = from_json(&result.unwrap()).unwrap();
        assert_eq!(response.vault_addresses.len(), 2);
        assert_eq!(response.vault_addresses[0], vault1);
        assert_eq!(response.vault_addresses[1], vault2);
    }
}
