

use crate::error::{ContractError, ContractResult};
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, UpdateConfig};
use crate::state::{Config, CONFIG};
use crate::utils::{*, validate_instantiate_msg, validate_update_config};
use crate::external_types::{AllApyResponse, CalculatedFeeTiers};
use cosmwasm_std::{attr, entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response, Addr, Decimal};
use cw2::set_contract_version;

use serde_json::to_vec;

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
    validate_instantiate_msg(&msg)?;

    // Set contract version for migration info
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let whitelist = msg
        .whitelist
        .iter()
        .map(|addr| deps.api.addr_validate(addr).map_err(ContractError::Std))
        .collect::<Result<Vec<Addr>, ContractError>>()?;

    let core_contracts = msg.assets.iter()
        .map(|c| deps.api.addr_validate(&c.core_contract).map_err(ContractError::from))
        .collect::<ContractResult<Vec<Addr>>>()?;
    let vault_addresses = msg.assets.iter()
        .map(|c| deps.api.addr_validate(&c.vault_address).map_err(ContractError::from))
        .collect::<ContractResult<Vec<Addr>>>()?;


    let apy_contract = deps.api.addr_validate(&msg.apy_contract)?;

    // Create and save config
    let config = Config {
        assets: msg.assets,
        apy_contract,
        whitelist,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("asset_count", config.assets.len().to_string())
        .add_attributes([
            attr("core_contracts", format!("{:?}", core_contracts)),
            attr("vault_addresses", format!("{:?}", vault_addresses)),
            attr("apy_contract", config.apy_contract.to_string()),
        ]))
}

///////////////
/// EXECUTE ///
///////////////

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig { new_config } => execute_update_config(deps, info, new_config),
        ExecuteMsg::RunVaultUpdate {} => execute_run_vault_update(deps, env, info)
    }
}
/////////////
/// QUERY ///
/////////////

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::GetConfig {} => {
            let config = CONFIG.load(deps.storage)?;
            let serialized_config = to_vec(&config).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_config))
        }
        QueryMsg::GetAllApy {} => {
            let config = CONFIG.load(deps.storage)?;
            let mut apys = Vec::<Decimal>::new();
            
            for asset in config.assets {
                match query_apy_contract(&deps, &config.apy_contract, &asset.core_contract, asset.query_period_hours) {
                    Ok(apy) => apys.push(apy),
                    Err(_e) => {
                        apys.push(Decimal::zero());
                        continue;
                    }
                }
            }
            
            let response = AllApyResponse { apys };
            let serialized_response = to_vec(&response).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_response))
        }
        QueryMsg::GetFeeTiers {} => {
            let config = CONFIG.load(deps.storage)?;
            let mut calculated_tiers = Vec::<CalculatedFeeTiers>::new();
            
            for asset in config.assets {
                let apy = match query_apy_contract(&deps, &config.apy_contract, &asset.core_contract, asset.query_period_hours) {
                    Ok(apy) => apy,
                    Err(_) => Decimal::zero(),
                };

                if apy.is_zero() {
                    // For zero APY, add entry with zeros
                    calculated_tiers.push(CalculatedFeeTiers {
                        denom: asset.denom,
                        apy,
                        base_fee: 0,
                        oracle_skew: 0,
                        fee_tiers: vec![],
                    });
                } else {
                    let base_fee = calculate_fee_tier(apy, asset.unbonding_period, asset.fee_dempening_amount)?;
                    let fee_tiers = create_fee_tiers(base_fee, &asset.fee_spacings, &asset.percentages)?;
                    let oracle_skew = (base_fee + 1) as i32;

                    // Convert to simple (fee, percentage) pairs
                    let fee_tier_pairs: Vec<(u64, u64)> = fee_tiers
                        .iter()
                        .map(|tier| (tier.fee, tier.percentage))
                        .collect();

                    calculated_tiers.push(CalculatedFeeTiers {
                        denom: asset.denom,
                        apy,
                        base_fee,
                        oracle_skew,
                        fee_tiers: fee_tier_pairs,
                    });
                }
            }
            
            let serialized_response = to_vec(&calculated_tiers).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_response))
        }
    }
}

///////////////
/// MIGRATE ///
///////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new().add_attribute("action", "migrate"))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_config: UpdateConfig,
) -> Result<Response, ContractError> {
    validate_update_config(&new_config)?;
    
    // Check if sender is in whitelist
    let mut config = CONFIG.load(deps.storage)?;
    if !config.whitelist.contains(&info.sender) {
        return Err(ContractError::Unauthorized);
    }

    // Update APY contract if provided
    if let Some(new_apy_contract) = new_config.new_apy_contract {

        let validated_apy_contract = deps.api.addr_validate(&new_apy_contract)?;
        config.apy_contract = validated_apy_contract;
    }

    // Update whitelist if provided
    if let Some(new_whitelist) = new_config.new_whitelist {
        let whitelist = new_whitelist
            .iter()
            .map(|addr| deps.api.addr_validate(addr).map_err(ContractError::Std))
            .collect::<Result<Vec<Addr>, ContractError>>()?;
        config.whitelist = whitelist;
    }

    // Update assets if provided
    if let Some(new_assets) = new_config.new_assets {

        for asset in &new_assets {
            deps.api.addr_validate(&asset.vault_address)?;
            deps.api.addr_validate(&asset.core_contract)?;
        }
        config.assets = new_assets;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("asset_count", config.assets.len().to_string())
        .add_attributes([
            attr("assets", format!("{:?}", config.assets)),
            attr("apy_contract", config.apy_contract.to_string()),
        ]))
}

fn execute_run_vault_update(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    
    // Check if sender is in whitelist
    if !config.whitelist.contains(&info.sender) {
        return Err(ContractError::Unauthorized);
    }
    let mut messages = Vec::new();
    let mut attributes = Vec::new();

    attributes.push(attr("action", "run_vault_update"));
    attributes.push(attr("vault_count", config.assets.len().to_string()));

    // Process each asset vault
    for asset in &config.assets {
        // Query APY for this asset
        let apy = query_apy_contract(
            &deps.as_ref(),
            &config.apy_contract,
            &asset.core_contract,
            asset.query_period_hours,
        )?;

        // Check if APY is zero
        let is_apy_zero: bool = apy.is_zero();

        if is_apy_zero {
            // If APY is zero, only perform withdrawal (no update or deposit)
            let withdrawal_msg = create_dex_withdrawal_message(&asset.vault_address)?;
            messages.push(withdrawal_msg);
            
            // zero APY case attrs
            attributes.push(attr(format!("vault_{}_apy", asset.denom), "0"));
            attributes.push(attr(format!("vault_{}_action", asset.denom), "withdrawal_only"));
            attributes.push(attr(format!("vault_{}_reason", asset.denom), "zero_apy"));
        } else {
            // if not zero apy, calculate base fee tier, create fee tiers, and update vault.
            // Calculate base fee tier using the APY and unbonding period
            let base_fee = calculate_fee_tier(apy, asset.unbonding_period, asset.fee_dempening_amount)?;

            // Create fee tiers by adding configured values to the calculated base fee
            let fee_tiers = create_fee_tiers(base_fee, &asset.fee_spacings, &asset.percentages)?;

            // Oracle skew is base fee + 1. can be counteracted with fee_spacing of 1 on the first tick index.
            let oracle_skew = (base_fee + 1) as i32;

            // Full sequence for all vaults: dex_withdrawal, update_config, dex_deposit
            let withdrawal_msg = create_dex_withdrawal_message(&asset.vault_address)?;
            let update_msg = create_vault_update_message(
                &asset.vault_address,
                &fee_tiers,
                oracle_skew,
                &info.sender.to_string(),
            )?;
            let deposit_msg = create_dex_deposit_message(&asset.vault_address)?;
            
            messages.push(withdrawal_msg);
            messages.push(update_msg);
            messages.push(deposit_msg);

            // Add attributes for this vault update
            attributes.push(attr(format!("vault_{}_apy", asset.denom), apy.to_string()));
            attributes.push(attr(format!("vault_{}_base_fee", asset.denom), base_fee.to_string()));
            attributes.push(attr(format!("vault_{}_oracle_skew", asset.denom), oracle_skew.to_string()));
            attributes.push(attr(format!("vault_{}_action", asset.denom), "full_update"));
        }
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(attributes))
}
