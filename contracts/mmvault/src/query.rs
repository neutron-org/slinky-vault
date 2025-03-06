use crate::error::ContractResult;
use crate::msg::CombinedPriceResponse;
use crate::state::CONFIG;
use crate::utils::*;
use cosmwasm_std::{to_json_binary, Binary, Deps, Env};
use neutron_std::types::neutron::dex::DexQuerier;

pub fn q_dex_deposit(deps: Deps, _env: Env) -> ContractResult<Binary> {
    let dex_querier = DexQuerier::new(&deps.querier);
    Ok(to_json_binary(&dex_querier.user_deposits_all(
        _env.contract.address.to_string(),
        None,
        true,
    )?)?)
}

pub fn query_config(deps: Deps, _env: Env) -> ContractResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    Ok(to_json_binary(&config)?)
}
