use crate::{error::ContractResult, utils::get_virtual_contract_balance};
use crate::state::CONFIG;
use cosmwasm_std::{to_json_binary, Binary, Deps, Env, Coin};
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

pub fn query_balance(deps: Deps, env: Env) -> ContractResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    let balance = get_virtual_contract_balance(env, deps, config.clone())?;
    let balance_coins = vec![
        Coin::new(balance.0, config.pair_data.token_0.denom),
        Coin::new(balance.1, config.pair_data.token_1.denom),
    ];
    Ok(to_json_binary(&balance_coins)?)

   
}
