use crate::error::ContractError;
use crate::msg::CombinedPriceResponse;
use crate::state::CONFIG;
use crate::{error::ContractResult, utils::get_virtual_contract_balance};
use cosmwasm_std::{to_json_binary, Addr, Binary, Coin, Deps, Env, Uint128};
use neutron_std::types::neutron::dex::DexQuerier;
use neutron_std::types::neutron::util::precdec::PrecDec;

use crate::utils::*;

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

pub fn simulate_provide_liquidity(
    deps: Deps,
    env: Env,
    amount_0: Uint128,
    amount_1: Uint128,
    sender: Addr,
) -> ContractResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    // get total contract balance, including value in active deposits.
    let (total_amount_0, total_amount_1) =
        get_virtual_contract_balance(env.clone(), deps, config.clone())?;

    let prices: CombinedPriceResponse = get_prices(deps, env.clone())?;

    // Get the value of the tokens in the contract
    let deposit_value = get_token_value(prices.clone(), amount_0, amount_1)?;
    let total_value = get_token_value(prices.clone(), total_amount_0, total_amount_1)?;

    // check if they exceed the cap, unless whitelisted
    let exceeds_cap = total_value > PrecDec::from_atomics(config.deposit_cap, 0).unwrap();
    if exceeds_cap && !config.whitelist.contains(&sender) {
        return Err(ContractError::ExceedsDepositCap {});
    }

    // get the amount of LP tokens to mint
    let amount_to_mint = get_mint_amount(config.clone(), deposit_value, total_value)?;
    Ok(to_json_binary(&amount_to_mint)?)
}

pub fn simulate_withdraw_liquidity(
    deps: Deps,
    env: Env,
    amount: Uint128,
) -> ContractResult<Binary> {
    let config = CONFIG.load(deps.storage)?;
    // get total contract balance, including value in active deposits.
    let (total_amount_0, total_amount_1) =
        get_virtual_contract_balance(env.clone(), deps, config.clone())?;
    let (_, withdraw_amount_0, withdraw_amount_1) = get_withdrawal_messages(
        &env,
        &config,
        amount,
        "neutron10h9stc5v6ntgeygf5xf945njqq5h32r54rf7kc".to_string(),
        total_amount_0,
        total_amount_1,
    )?;

    let response = (withdraw_amount_0, withdraw_amount_1);
    Ok(to_json_binary(&response)?)
}
