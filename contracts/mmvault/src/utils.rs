use crate::error::{ContractError, ContractResult};
use crate::msg::{CombinedPriceResponse, DepositResult};
use crate::state::{Config, PairData, TokenData, CONFIG, SHARES_MULTIPLIER};
use cosmwasm_std::{
    BalanceResponse, BankMsg, BankQuery, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, QueryRequest,
    ReplyOn, SubMsg, SubMsgResponse, Uint128,
};
use neutron_std::types::neutron::dex::{
    DepositOptions, DexQuerier, MsgDeposit, MsgWithdrawal, MsgWithdrawalResponse,
    QueryAllUserDepositsResponse,
};
use neutron_std::types::neutron::util::precdec::PrecDec;
use neutron_std::types::osmosis::tokenfactory::v1beta1::MsgBurn;
use neutron_std::types::osmosis::tokenfactory::v1beta1::MsgCreateDenomResponse;

use prost::Message;

pub fn sort_token_data_and_get_pair_id_str(
    token0: &TokenData,
    token1: &TokenData,
) -> ([TokenData; 2], String) {
    let mut tokens = [token0.clone(), token1.clone()];
    if token1.denom < token0.denom {
        tokens.reverse();
    }
    (
        tokens.clone(),
        [tokens[0].denom.clone(), tokens[1].denom.clone()].join("<>"),
    )
}

pub fn get_prices(deps: Deps, _env: Env) -> ContractResult<CombinedPriceResponse> {
    let config = CONFIG.load(deps.storage)?;

    let prices: CombinedPriceResponse = deps.querier.query_wasm_smart(
        config.oracle_contract,
        &serde_json::json!({
            "get_prices": {
                "token_a": config.pair_data.token_0,
                "token_b": config.pair_data.token_1,
            }
        }),
    )?;

    Ok(prices)
}

pub fn get_token_value(
    prices: CombinedPriceResponse,
    token0_deposited: Uint128,
    token1_deposited: Uint128,
) -> ContractResult<(PrecDec, PrecDec)> {
    let mut value_0 = PrecDec::zero();
    let mut value_1 = PrecDec::zero();

    if !token0_deposited.is_zero() {
        value_0 = PrecDec::from_atomics(token0_deposited, 0)
            .unwrap()
            .checked_mul(prices.token_0_price)?;
    }
    if !token1_deposited.is_zero() {
        value_1 = PrecDec::from_atomics(token1_deposited, 0)
            .unwrap()
            .checked_mul(prices.token_1_price)?;
    }

    Ok((value_0, value_1))
}

/// Queries the contract's balance for the specified token denoms
pub fn query_contract_balance(
    deps: &DepsMut,
    env: Env,
    pair_data: PairData,
) -> Result<Vec<Coin>, ContractError> {
    let contract_address = env.contract.address;
    let mut balances: Vec<Coin> = vec![];

    for denom in &[pair_data.token_0.denom, pair_data.token_1.denom] {
        let balance_request = QueryRequest::Bank(BankQuery::Balance {
            address: contract_address.to_string(),
            denom: denom.clone(),
        });

        // Query the balance for each denom
        let balance_resp: BalanceResponse = deps.querier.query(&balance_request)?;

        // Add the balance to the balances vector
        balances.push(Coin {
            denom: denom.clone(),
            amount: balance_resp.amount.amount,
        });
    }

    Ok(balances)
}


pub fn price_to_tick_index(price: PrecDec) -> Result<i64, ContractError> {
    // Ensure the price is greater than 0
    if price.is_zero() || price < PrecDec::zero() {
        return Err(ContractError::InvalidPrice);
    }

    // Convert PrecDec to f64
    let price_f64 = price
        .to_string()
        .parse::<f64>()
        .map_err(|_| ContractError::ConversionError)?;

    // Compute the logarithm of the base (1.0001)
    let log_base = 1.0001f64.ln();

    // Compute the logarithm of the price
    let log_price = price_f64.ln();

    // Calculate the tick index using the formula: TickIndex = -log(Price) / log(1.0001)
    let tick_index = -(log_price / log_base);

    // Convert the tick index to i64, rounding to the nearest integer
    Ok(tick_index.round() as i64)
}

pub fn get_deposit_data(
    total_available_0: Uint128,
    total_available_1: Uint128,
    tick_index: i64,
    fee: u64,
    prices: &CombinedPriceResponse,
    base_deposit_percentage: u64,
    skew: bool,
) -> Result<DepositResult, ContractError> {
    // Calculate the base deposit amounts

    let computed_amount_0 = total_available_0.multiply_ratio(base_deposit_percentage, 100u128);
    // Calculate value in USD for token 0
    let value_token_0 = PrecDec::from_atomics(total_available_0 - computed_amount_0, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        .checked_mul(prices.token_0_price)?;

    let computed_amount_1 = total_available_1.multiply_ratio(base_deposit_percentage, 100u128);
    // Calculate value in USD for token 1
    let value_token_1 = PrecDec::from_atomics(total_available_1 - computed_amount_1, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        .checked_mul(prices.token_1_price)?;

    let (final_amount_0, final_amount_1) = if value_token_0 > value_token_1 {
        let imbalance = (value_token_0 - value_token_1) * PrecDec::percent(50);
        let additional_token_0 = imbalance / prices.token_0_price;
        (
            computed_amount_0
                + Uint128::try_from(additional_token_0.to_uint_floor())
                    .map_err(|_| ContractError::ConversionError)?,
            computed_amount_1,
        )
    } else if value_token_1 > value_token_0 {
        let imbalance = (value_token_1 - value_token_0).checked_mul(PrecDec::percent(50))?;
        let additional_token_1 = imbalance.checked_div(prices.token_1_price)?;
        (
            computed_amount_0,
            computed_amount_1
                + Uint128::try_from(additional_token_1.to_uint_floor())
                    .map_err(|_| ContractError::ConversionError)?,
        )
    } else {
        (computed_amount_0, computed_amount_1)
    };

    let final_amount_0 = if final_amount_0 > total_available_0 {
        total_available_0
    } else {
        final_amount_0
    };

    let final_amount_1 = if final_amount_1 > total_available_1 {
        total_available_1
    } else {
        final_amount_1
    };
    
    // Calculate adjusted tick index based on token value imbalance
    let adjusted_tick_index = if skew {
        calculate_adjusted_tick_index(
            tick_index,
            fee,
            value_token_0,
            value_token_1,
        )?
    } else {
        tick_index
    };

    let result = DepositResult {
        amount0: final_amount_0,
        amount1: final_amount_1,
        tick_index: adjusted_tick_index,
        fee,
    };
    Ok(result)
}

/// Calculates an adjusted tick index based on token value imbalance
/// 
/// If values are balanced, no adjustment is made
/// If token0 value dominates tick index is linearly increased (up to fee-1)
/// If token1 value dominates tick index is linearly decreased (up to fee-1)
pub fn calculate_adjusted_tick_index(
    base_tick_index: i64,
    fee: u64,
    value_token_0: PrecDec,
    value_token_1: PrecDec,
) -> Result<i64, ContractError> {
    // If either value is zero, handle the edge cases
    if value_token_0.is_zero() && value_token_1.is_zero() {
        return Ok(base_tick_index); // No adjustment if both values are zero
    }

    // Calculate the maximum tick adjustment (fee-1)
    let max_adjustment = (fee as i64) - 1;
    if max_adjustment <= 0 {
        return Ok(base_tick_index); // No adjustment possible if fee <= 1
    }
    
    // Calculate the total value
    let total_value = value_token_0.checked_add(value_token_1)?;
    
    // Handle edge cases
    if value_token_0.is_zero() {
        // Token1 completely dominates, move tick down by max_adjustment
        return Ok(base_tick_index - max_adjustment);
    }
    
    if value_token_1.is_zero() {
        // Token0 completely dominates, move tick up by max_adjustment
        return Ok(base_tick_index + max_adjustment);
    }
    
    // Calculate the imbalance ratio (-1.0 to 1.0)
    // -1.0 means token1 completely dominates
    // 1.0 means token0 completely dominates
    // 0.0 means perfectly balanced
    let imbalance = value_token_0
        .checked_sub(value_token_1)?
        .checked_div(total_value)?;
    
    // Convert the imbalance to a tick adjustment
    // We need to convert PrecDec to f64 for the calculation
    let imbalance_f64 = imbalance
        .to_string()
        .parse::<f64>()
        .map_err(|_| ContractError::ConversionError)?;
    
    // Calculate the adjustment linearly based on the imbalance
    let adjustment = (imbalance_f64 * max_adjustment as f64).round() as i64;
    
    // Apply the adjustment to the base tick index
    Ok(base_tick_index + adjustment)
}

pub fn extract_withdrawal_amounts(
    result: &SubMsgResponse,
) -> Result<(Uint128, Uint128), ContractError> {
    let response_data = result
        .msg_responses
        .first()
        .ok_or(ContractError::NoResponseData)?
        .value
        .clone();

    let withdrawal = MsgWithdrawalResponse::decode(response_data.as_slice())
        .map_err(|_| ContractError::DecodingError)?;

    let amount0 = withdrawal
        .reserve0_withdrawn
        .parse::<Uint128>()
        .map_err(|_| ContractError::DecodingError)?;

    let amount1 = withdrawal
        .reserve1_withdrawn
        .parse::<Uint128>()
        .map_err(|_| ContractError::DecodingError)?;

    Ok((amount0, amount1))
}

pub fn extract_denom(result: &SubMsgResponse) -> Result<String, ContractError> {
    let response_data = result
        .msg_responses
        .first()
        .ok_or(ContractError::NoResponseData)?
        .value
        .clone();

    let response = MsgCreateDenomResponse::decode(response_data.as_slice())
        .map_err(|_| ContractError::DecodingError)?;

    let denom = response.new_token_denom;

    Ok(denom)
}
pub fn get_virtual_contract_balance(
    env: Env,
    deps: &DepsMut,
    config: Config,
) -> Result<(Uint128, Uint128), ContractError> {
    let dex_querier = DexQuerier::new(&deps.querier);
    // simulate full withdrawal to get the current total token amounts:
    let res: QueryAllUserDepositsResponse =
        dex_querier.user_deposits_all(env.contract.address.to_string(), None, true)?;
    // If there are any active deposits, withdraw all of them

    let balances = query_contract_balance(deps, env.clone(), config.pair_data.clone())?;
    let mut total_amount_0 = balances[0].amount;
    let mut total_amount_1 = balances[1].amount;

    for deposit in res.deposits.iter() {
        let withdraw_msg = MsgWithdrawal {
            creator: env.contract.address.to_string(),
            receiver: env.contract.address.to_string(),
            token_a: config.pair_data.token_0.denom.clone(),
            token_b: config.pair_data.token_1.denom.clone(),
            shares_to_remove: vec![deposit
                .shares_owned
                .parse()
                .expect("Failed to parse the string as an integer")],
            tick_indexes_a_to_b: vec![deposit.center_tick_index],
            fees: vec![deposit.fee],
        };

        // Wrap the DexMsg into a SubMsg with reply
        let sim_response = dex_querier.simulate_withdrawal(Some(withdraw_msg))?;
        let amount_0 = sim_response
            .resp
            .clone()
            .unwrap()
            .reserve0_withdrawn
            .parse::<Uint128>()
            .unwrap();
        let amount_1 = sim_response
            .resp
            .clone()
            .unwrap()
            .reserve1_withdrawn
            .parse::<Uint128>()
            .unwrap();
        total_amount_0 += amount_0;
        total_amount_1 += amount_1;
    }
    Ok((total_amount_0, total_amount_1))
}

pub fn get_mint_amount(
    env: Env,
    deps: &DepsMut,
    config: Config,
    prices: CombinedPriceResponse,
    deposited_value_token_0: PrecDec,
    deposited_value_token_1: PrecDec,
) -> Result<Uint128, ContractError> {
    let mut total_shares = PrecDec::zero();
    let balances = query_contract_balance(deps, env.clone(), config.pair_data.clone())?;

    //get total contract balance:
    let (total_amount_0, total_amount_1) = get_virtual_contract_balance(env, deps, config.clone())?;

    // Get the total value of the remaining tokens
    let total_value_token_0 = PrecDec::from_atomics(total_amount_0, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        .checked_mul(prices.token_0_price)?;
    let total_value_token_1 = PrecDec::from_atomics(total_amount_1, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        .checked_mul(prices.token_1_price)?;

    let total_value_combined = total_value_token_0.checked_add(total_value_token_1)?;
    let deposit_value_incoming = deposited_value_token_0
        .checked_add(deposited_value_token_1)
        .unwrap();

    if config.total_shares == Uint128::zero() {
        // Initial deposit - set shares equal to deposit value
        total_shares = deposit_value_incoming.checked_mul(PrecDec::from_ratio(SHARES_MULTIPLIER, 1u128))?;
    } else {
        // Calculate proportional shares based on the ratio of deposit value to total value
        total_shares = deposit_value_incoming
            .checked_mul(PrecDec::from_ratio(config.total_shares, 1u128))
            .map_err(|_| ContractError::ConversionError)?
            .checked_div(total_value_combined)
            .map_err(|_| ContractError::ConversionError)?;
    }

    if total_shares.is_zero() {
        return Err(ContractError::InvalidTokenAmount);
    }

    Ok(precdec_to_uint128(total_shares)?)
}

pub fn precdec_to_uint128(precdec: PrecDec) -> Result<Uint128, ContractError> {
    // Check if the value is negative
    if precdec < PrecDec::zero() {
        return Err(ContractError::ConversionError);
    }

    // Convert to uint256 floor value to handle potential overflow
    let uint_floor = precdec.to_uint_floor();

    // Check if the value exceeds Uint128::MAX
    if uint_floor > Uint128::MAX.into() {
        return Err(ContractError::ConversionError);
    }
    let as_u128: Uint128 = uint_floor
        .try_into()
        .map_err(|_| ContractError::ConversionError)?;

    Ok(as_u128)
}

pub fn get_deposit_messages(
    env: &Env,
    config: Config,
    tick_index: i64,
    prices: crate::msg::CombinedPriceResponse,
    token_0_balance: Uint128,
    token_1_balance: Uint128,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let mut messages = Vec::new();

    // get the amount to deposit at the tightest spread
    let deposit_data = get_deposit_data(
        token_0_balance,
        token_1_balance,
        tick_index,
        config.fee_tier_config.fee_tiers[0].fee,
        &prices,
        config.fee_tier_config.fee_tiers[0].percentage,
        config.skew,
    )?;

    // Only create base deposit message if amounts are greater than zero
    if deposit_data.amount0 > Uint128::zero() || deposit_data.amount1 > Uint128::zero() {
        let dex_msg = Into::<CosmosMsg>::into(MsgDeposit {
            creator: env.contract.address.to_string(),
            receiver: env.contract.address.to_string(),
            token_a: config.pair_data.token_0.denom.clone(),
            token_b: config.pair_data.token_1.denom.clone(),
            amounts_a: vec![deposit_data.amount0.to_string()],
            amounts_b: vec![deposit_data.amount1.to_string()],
            tick_indexes_a_to_b: vec![deposit_data.tick_index],
            fees: vec![deposit_data.fee],
            options: vec![DepositOptions {
                disable_autoswap: false,
                fail_tx_on_bel: false,
            }],
        });
        messages.push(dex_msg);
    }

    // Calculate remaining amounts for ambient deposits
    let remaining_amount0 = token_0_balance
        .checked_sub(deposit_data.amount0)
        .unwrap_or(Uint128::zero());
    let remaining_amount1 = token_1_balance
        .checked_sub(deposit_data.amount1)
        .unwrap_or(Uint128::zero());

    // get the remaining deposit messages
    for fee_tier in config.fee_tier_config.fee_tiers.iter() {
        let amount_0 = remaining_amount0.multiply_ratio(fee_tier.percentage, 100u128);
        let amount_1 = remaining_amount1.multiply_ratio(fee_tier.percentage, 100u128);
        if amount_0 > Uint128::zero() || amount_1 > Uint128::zero() {
            let dex_msg = Into::<CosmosMsg>::into(MsgDeposit {
                creator: env.contract.address.to_string(),
                receiver: env.contract.address.to_string(),
                token_a: config.pair_data.token_0.denom.clone(),
                token_b: config.pair_data.token_1.denom.clone(),
                amounts_a: vec![amount_0.to_string()],
                amounts_b: vec![amount_1.to_string()],
                tick_indexes_a_to_b: vec![deposit_data.tick_index],
                fees: vec![fee_tier.fee],
                options: vec![DepositOptions {
                    disable_autoswap: false,
                    fail_tx_on_bel: false,
                }],
            });
            messages.push(dex_msg);
        }
    }

    Ok(messages)
}

/// Takes a vector of CosmosMsg vectors and returns a vector of SubMsg where only the last message has a reply.
/// Returns an error if messages is empty.
pub fn flatten_msgs_always_reply(
    messages: &[Vec<CosmosMsg>],
    reply_id: u64,
    payload: Option<Binary>,
) -> Result<Vec<SubMsg>, ContractError> {
    let mut submsgs: Vec<SubMsg> = messages.concat().into_iter().map(SubMsg::new).collect();

    if submsgs.is_empty() {
        return Err(ContractError::NoFundsAvailable {});
    }

    // Add reply to the last message
    if let Some(last) = submsgs.last_mut() {
        last.id = reply_id;
        last.reply_on = ReplyOn::Success;
        last.payload = payload.unwrap_or_default();
    }

    Ok(submsgs)
}

pub fn get_withdrawal_messages(
    env: &Env,
    deps: &DepsMut,
    config: &Config,
    burn_amount: Uint128,
    beneficiary: String,
) -> Result<(Vec<CosmosMsg>, Uint128, Uint128), ContractError> {
    let mut messages: Vec<CosmosMsg> = vec![];

    let balances = get_virtual_contract_balance(env.clone(), deps, config.clone())?;
    let total_supply: Uint128 = deps.querier.query_supply(&config.lp_denom)?.amount;
    // Calculate withdrawal amounts using multiplication before division to prevent precision loss
    // and potential overflow
    let withdraw_amount_0 = balances.0.multiply_ratio(burn_amount, total_supply);
    let withdraw_amount_1 = balances.1.multiply_ratio(burn_amount, total_supply);

    // burn the LP tokens
    let burn_msg = MsgBurn {
        sender: env.contract.address.to_string(),
        amount: Some(
            Coin {
                denom: config.lp_denom.clone(),
                amount: burn_amount,
            }
            .into(),
        ),
        burn_from_address: env.contract.address.to_string(),
    };

    messages.push(burn_msg.into());

    if !(balances.0.is_zero() && withdraw_amount_0.is_zero()) {
        messages.push(
            BankMsg::Send {
                to_address: beneficiary.clone(),
                amount: vec![Coin {
                    denom: config.pair_data.token_0.denom.clone(),
                    amount: withdraw_amount_0,
                }],
            }
            .into(),
        );
    }

    if !(balances.1.is_zero() && withdraw_amount_1.is_zero()) {
        messages.push(
            BankMsg::Send {
                to_address: beneficiary.clone(),
                amount: vec![Coin {
                    denom: config.pair_data.token_1.denom.clone(),
                    amount: withdraw_amount_1,
                }],
            }
            .into(),
        );
    }
    Ok((messages, withdraw_amount_0, withdraw_amount_1))
}
