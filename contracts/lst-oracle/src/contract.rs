use crate::error::{ContractError, ContractResult};
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, UpdateConfig};
use crate::state::{Config, CONFIG};
use crate::utils::*;
use cosmwasm_std::{attr, entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;
use cw_ownable::update_ownership;
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

    let config = &Config {
        lst_asset_denom: msg.lst_asset_denom,
        redemption_rate: msg.redemption_rate,
    };

    CONFIG.save(deps.storage, config)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attributes([attr(
            "redemption rate",
            format!("{:?}", config.redemption_rate),
        )]))
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
            let redemption_rate = CONFIG.load(deps.storage)?.redemption_rate;
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

    if let Some(d_asset_denom) = new_config.lst_asset_denom {
        config.lst_asset_denom = d_asset_denom;
    }
    if let Some(redemption_rate) = new_config.redemption_rate {
        config.redemption_rate = redemption_rate;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attributes([
            attr("d_asset_base", format!("{:?}", config.lst_asset_denom)),
            attr("redemption_rate", format!("{:?}", config.redemption_rate)),
        ]))
}
