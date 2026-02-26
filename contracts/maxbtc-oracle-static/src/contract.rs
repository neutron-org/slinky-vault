use crate::error::{ContractError, ContractResult};
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, UpdateConfig};
use crate::state::{Config, CONFIG};
use crate::utils::*;
use cosmwasm_std::{attr, entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;
use serde_json::to_vec;
use std::str::FromStr;
use neutron_std::types::neutron::util::precdec::PrecDec;

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
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    
    // Validate and convert initial owners
    let mut owners = Vec::new();
    for owner_str in msg.initial_owners {
        let owner = deps.api.addr_validate(&owner_str)?;
        owners.push(owner);
    }
    
    let config = &Config {
        owners,
        maxbtc_redemption_rate: msg.maxbtc_redemption_rate,
        maxbtc_denom: msg.maxbtc_denom,
        lst_denom: msg.lst_denom,
        lst_redemption_rate: msg.lst_redemption_rate,
    };

    CONFIG.save(deps.storage, config)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attributes([
            attr("maxbtc_redemption_rate", format!("{:?}", config.maxbtc_redemption_rate)),
            attr("maxbtc_denom", format!("{:?}", config.maxbtc_denom)),
            attr("lst_denom", format!("{:?}", config.lst_denom)),
            attr("lst_redemption_rate", format!("{:?}", config.lst_redemption_rate)),
        ]))
}

///////////////
/// EXECUTE ///
///////////////

#[cfg_attr(not(feature = "library"), cosmwasm_std::entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig { new_config } => execute_update_config(deps, info, new_config),
    }
}

/////////////
/// QUERY ///
/////////////

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::GetPrices { token_a, token_b } => {
            let prices = get_prices(deps, _env, token_a, token_b)?;
            let serialized_prices =
                to_vec(&prices).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_prices))
        }
        QueryMsg::GetRedemptionRates {} => {
            let dual_redemption_rates = query_dual_redemption_rates(deps, _env)?;
            let serialized_rates =
                to_vec(&dual_redemption_rates).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_rates))
        }
        QueryMsg::GetMaxBtcRedemptionRate {} => {
            let config = CONFIG.load(deps.storage)?;
            let maxbtc_redemption_rate_str = PrecDec::to_string(&config.maxbtc_redemption_rate);
            let maxbtc_redemption_rate = cosmwasm_std::Decimal::from_str(&maxbtc_redemption_rate_str)
            .map_err(|_| ContractError::DecimalConversionError)?;
            let serialized_rate =
                to_vec(&maxbtc_redemption_rate).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_rate))
        }
        QueryMsg::GetLstRedemptionRate {} => {
            let config = CONFIG.load(deps.storage)?;
            let lst_redemption_rate_str = PrecDec::to_string(&config.lst_redemption_rate);
            let lst_redemption_rate = cosmwasm_std::Decimal::from_str(&lst_redemption_rate_str)
                .map_err(|_| ContractError::DecimalConversionError)?;
            let serialized_rate =
                to_vec(&lst_redemption_rate).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_rate))
        }
        QueryMsg::GetMaxBtcDenom {} => {
            let maxbtc_denom = CONFIG.load(deps.storage)?.maxbtc_denom;
            let serialized_denom =
                to_vec(&maxbtc_denom).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_denom))
        }
        QueryMsg::GetLstDenom {} => {
            let lst_denom = CONFIG.load(deps.storage)?.lst_denom;
            let serialized_denom =
                to_vec(&lst_denom).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_denom))
        }
        QueryMsg::GetOwners {} => {
            let config = CONFIG.load(deps.storage)?;
            let owners: Vec<String> = config.owners.iter().map(|addr| addr.to_string()).collect();
            let serialized_owners =
                to_vec(&owners).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_owners))
        }
        QueryMsg::IsOwner { address } => {
            let addr = deps.api.addr_validate(&address)?;
            let config = CONFIG.load(deps.storage)?;
            let is_owner = config.owners.contains(&addr);
            let serialized_result =
                to_vec(&is_owner).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_result))
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
    let mut config = CONFIG.load(deps.storage)?;

    // Check if sender is an owner
    if !config.owners.contains(&info.sender) {
        return Err(ContractError::Unauthorized);
    }

    if let Some(owners) = new_config.owners {
        let mut validated_owners = Vec::new();
        for owner_str in owners {
            let owner = deps.api.addr_validate(&owner_str)?;
            validated_owners.push(owner);
        }
        config.owners = validated_owners;
    }
    if let Some(maxbtc_redemption_rate) = new_config.maxbtc_redemption_rate {
        config.maxbtc_redemption_rate = maxbtc_redemption_rate;
    }
    if let Some(maxbtc_denom) = new_config.maxbtc_denom {
        config.maxbtc_denom = maxbtc_denom;
    }
    if let Some(lst_denom) = new_config.lst_denom {
        config.lst_denom = lst_denom;
    }
    if let Some(lst_redemption_rate) = new_config.lst_redemption_rate {
        config.lst_redemption_rate = lst_redemption_rate;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attributes([
            attr("owners", format!("{:?}", config.owners)),
            attr("maxbtc_redemption_rate", format!("{:?}", config.maxbtc_redemption_rate)),
            attr("maxbtc_denom", format!("{:?}", config.maxbtc_denom)),
            attr("lst_denom", format!("{:?}", config.lst_denom)),
            attr("lst_redemption_rate", format!("{:?}", config.lst_redemption_rate)),
        ]))
}

