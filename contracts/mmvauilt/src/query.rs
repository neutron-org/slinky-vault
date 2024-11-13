use crate::error::{ContractError, ContractResult};
use crate::msg::CombinedPriceResponse;
use crate::utils::*;
use cosmwasm_std::{to_json_binary, Binary, Deps, Env,};
use neutron_std::types::neutron::dex::DexQuerier;

pub fn query_recent_valid_prices_formatted(
    deps: Deps,
    env: Env,
) -> ContractResult<Binary> {
    let combined_responce: CombinedPriceResponse = get_prices(deps, env)?;

    return Ok(to_json_binary(&combined_responce)?);
}

pub fn q_dex_deposit(deps: Deps, _env: Env) -> ContractResult<Binary> {
    let dex_querier = DexQuerier::new(&deps.querier);
    Ok(to_json_binary(&dex_querier.user_deposits_all(
        _env.contract.address.to_string(),
        None,
        true,
    )?)?)
}

