use crate::error::{ContractError, ContractResult};
use crate::external_types::{QueryMsgDrop, RedemptionRateResponse};
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, UpdateConfig};
use crate::state::{Config, CONFIG};
use crate::utils::*;
use cosmwasm_std::{
    attr, entry_point, to_json_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
};
use cw2::{get_contract_version, set_contract_version};
use cw_ownable::{get_ownership, update_ownership};
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
    // Set contract version for migration info
    cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    cw_ownable::initialize_owner(deps.storage, deps.api, Some(msg.owner.as_ref()))?;

    let core_contract = deps.api.addr_validate(&msg.core_contract)?;
    let config = &Config {
        core_contract: core_contract.clone(),
        d_asset_denom: msg.d_asset_denom,
    };

    CONFIG.save(deps.storage, config)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attributes([attr("core contract", format!("{:?}", config.core_contract))]))
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
        ExecuteMsg::UpdateOwnership(action) => {
            update_ownership(deps.into_empty(), &env.block, &info.sender, action)
                .map_err(|_| ContractError::UpdateOwnershipError)?;
            Ok(Response::new())
        }
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
        QueryMsg::GetRedemptionRate {} => {
            let redemption_rate = query_redemption_rate(deps, _env)?;
            let serialized_redemption_rate =
                to_vec(&redemption_rate).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_redemption_rate))
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
    cw_ownable::assert_owner(deps.storage, &info.sender)
        .map_err(|_| ContractError::UpdateOwnershipError)?;
    let mut config = CONFIG.load(deps.storage)?;

    if let Some(core_contract) = new_config.core_contract {
        let new_core_contract = deps.api.addr_validate(&core_contract)?;
        config.core_contract = new_core_contract;
    }
    if let Some(d_asset_denom) = new_config.d_asset_denom {
        config.d_asset_denom = d_asset_denom;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attributes([
            attr("core contract", format!("{:?}", config.core_contract)),
            attr("d_asset_base", format!("{:?}", config.d_asset_denom)),
        ]))
}
