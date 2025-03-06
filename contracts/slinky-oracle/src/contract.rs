use crate::error::{ContractError, ContractResult};
use crate::msg::{QueryMsg, MigrateMsg, InstantiateMsg};
use cosmwasm_std::{
    attr, entry_point, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Reply, Response, Uint128, Addr, BankMsg, CosmosMsg, SubMsg, StdResult
};
use cw2::{set_contract_version, get_contract_version};
use prost::Message;
use crate::utils::*;
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
    _msg: InstantiateMsg,
) -> ContractResult<Response> {
    // Set contract version for migration info
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    
    Ok(Response::new().add_attributes(vec![
        attr("method", "instantiate"),
        attr("contract_name", CONTRACT_NAME),
        attr("contract_version", CONTRACT_VERSION),
    ]))
}


/////////////
/// QUERY ///
/////////////

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::GetPrices { token_a, token_b } => {
            let prices = get_prices(deps, _env, token_a, token_b)?;
            let serialized_prices = to_vec(&prices).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_prices))
        }
    }
}

//////////////
/// MIGRATE //
//////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> ContractResult<Response> {
    let current_version = get_contract_version(deps.storage)?;
    
    // Ensure we're migrating from a previous version
    if current_version.contract != CONTRACT_NAME {
        return Err(ContractError::InvalidMigration {
            previous_contract: current_version.contract,
        });
    }

    // Update contract version
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(Response::new().add_attributes(vec![
        attr("method", "migrate"),
        attr("previous_version", current_version.version),
        attr("new_version", CONTRACT_VERSION),
    ]))
}