use crate::error::{ContractError, ContractResult};
use cosmwasm_std::{to_json_binary, Addr, CosmosMsg, WasmMsg};
use serde_json::json;

/// Create a dex_withdrawal message for a vault contract
pub fn create_dex_withdrawal_message(vault_address: &Addr) -> ContractResult<CosmosMsg> {
    let msg = json!({"dex_withdrawal": {}});
    
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: vault_address.to_string(),
        msg: to_json_binary(&msg).map_err(|_| ContractError::SerializationError)?,
        funds: vec![],
    }))
}

/// Create a dex_deposit message for a vault contract
pub fn create_dex_deposit_message(vault_address: &Addr) -> ContractResult<CosmosMsg> {
    let msg = json!({"dex_deposit": {}});
    
    Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: vault_address.to_string(),
        msg: to_json_binary(&msg).map_err(|_| ContractError::SerializationError)?,
        funds: vec![],
    }))
}

/// Validate vault addresses to ensure they are unique and non-empty
pub fn validate_vault_addresses(vault_addresses: &[String]) -> ContractResult<()> {
    if vault_addresses.is_empty() {
        return Err(ContractError::NoVaultsConfigured);
    }

    // Check for empty addresses
    for (i, addr) in vault_addresses.iter().enumerate() {
        if addr.trim().is_empty() {
            return Err(ContractError::InvalidVaultAddress {
                addr: format!("address at index {}", i),
            });
        }
    }

    // Check for duplicates
    let mut seen = std::collections::HashSet::new();
    for addr in vault_addresses {
        if !seen.insert(addr) {
            return Err(ContractError::DuplicateVaultAddress {
                addr: addr.clone(),
            });
        }
    }

    Ok(())
}

/// Validate a single address string is not empty
pub fn validate_address(address: &str, field_name: &str) -> ContractResult<()> {
    if address.trim().is_empty() {
        return Err(ContractError::EmptyValue {
            kind: field_name.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::Addr;

    #[test]
    fn test_create_dex_withdrawal_message() {
        let vault_addr = Addr::unchecked("neutron1test_vault");
        let msg = create_dex_withdrawal_message(&vault_addr).unwrap();
        
        match msg {
            CosmosMsg::Wasm(WasmMsg::Execute { contract_addr, msg: _, funds }) => {
                assert_eq!(contract_addr, "neutron1test_vault");
                assert!(funds.is_empty());
            }
            _ => panic!("Expected WasmMsg::Execute"),
        }
    }

    #[test]
    fn test_create_dex_deposit_message() {
        let vault_addr = Addr::unchecked("neutron1test_vault");
        let msg = create_dex_deposit_message(&vault_addr).unwrap();
        
        match msg {
            CosmosMsg::Wasm(WasmMsg::Execute { contract_addr, msg: _, funds }) => {
                assert_eq!(contract_addr, "neutron1test_vault");
                assert!(funds.is_empty());
            }
            _ => panic!("Expected WasmMsg::Execute"),
        }
    }

    #[test]
    fn test_validate_vault_addresses_valid() {
        let vaults = vec![
            "neutron1vault1".to_string(),
            "neutron1vault2".to_string(),
            "neutron1vault3".to_string(),
        ];
        
        let result = validate_vault_addresses(&vaults);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_vault_addresses_empty_list() {
        let vaults: Vec<String> = vec![];
        
        let result = validate_vault_addresses(&vaults);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ContractError::NoVaultsConfigured => {},
            _ => panic!("Expected NoVaultsConfigured error"),
        }
    }

    #[test]
    fn test_validate_vault_addresses_empty_address() {
        let vaults = vec![
            "neutron1vault1".to_string(),
            "".to_string(),
            "neutron1vault3".to_string(),
        ];
        
        let result = validate_vault_addresses(&vaults);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ContractError::InvalidVaultAddress { .. } => {},
            _ => panic!("Expected InvalidVaultAddress error"),
        }
    }

    #[test]
    fn test_validate_vault_addresses_duplicates() {
        let vaults = vec![
            "neutron1vault1".to_string(),
            "neutron1vault2".to_string(),
            "neutron1vault1".to_string(), // duplicate
        ];
        
        let result = validate_vault_addresses(&vaults);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ContractError::DuplicateVaultAddress { addr } => {
                assert_eq!(addr, "neutron1vault1");
            },
            _ => panic!("Expected DuplicateVaultAddress error"),
        }
    }

    #[test]
    fn test_validate_address_valid() {
        let result = validate_address("neutron1test_address", "test_field");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_address_empty() {
        let result = validate_address("", "test_field");
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ContractError::EmptyValue { kind } => {
                assert_eq!(kind, "test_field");
            },
            _ => panic!("Expected EmptyValue error"),
        }
    }

    #[test]
    fn test_validate_address_whitespace_only() {
        let result = validate_address("   ", "test_field");
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ContractError::EmptyValue { kind } => {
                assert_eq!(kind, "test_field");
            },
            _ => panic!("Expected EmptyValue error"),
        }
    }
}
