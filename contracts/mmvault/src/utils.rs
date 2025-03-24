use crate::error::{ContractError, ContractResult};
use crate::msg::{CombinedPriceResponse, DepositResult};
use crate::state::{Config, PairData, TokenData, CONFIG, SHARES_MULTIPLIER};
use cosmwasm_std::{
    BalanceResponse, BankMsg, BankQuery, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    QueryRequest, ReplyOn, Response, SubMsg, SubMsgResponse, Uint128,
};
use neutron_std::types::neutron::dex::{
    DepositOptions, DexQuerier, LimitOrderType, MsgDeposit, MsgPlaceLimitOrder, MsgWithdrawal,
    MsgWithdrawalResponse, QueryAllUserDepositsResponse,
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

    let prices: CombinedPriceResponse = deps
        .querier
        .query_wasm_smart(
            config.oracle_contract,
            &serde_json::json!({
                "get_prices": {
                    "token_a": config.pair_data.token_0,
                    "token_b": config.pair_data.token_1,
                }
            }),
        )
        .map_err(|e| ContractError::OracleError {
            msg: format!("Failed to query oracle: {}", e),
        })?;

    Ok(prices)
}
/// Get the value of the tokens in USD
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

/// Converts a price to a tick index.
/// This is used to calculate the tick index for the AMM Deposit.
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

/// Get the amount of tokens to deposit at the tightest spread.
/// This is called on the lowest fee-tier, and will return the amount of tokens to deposit.
pub fn get_deposit_data(
    total_available_0: Uint128,
    total_available_1: Uint128,
    tick_index: i64,
    fee: u64,
    prices: &CombinedPriceResponse,
    base_deposit_percentage: u64,
    skew: bool,
    config_imbalance: u32,
) -> Result<DepositResult, ContractError> {
    let computed_amount_0 = total_available_0.multiply_ratio(base_deposit_percentage, 100u128);
    let computed_amount_1 = total_available_1.multiply_ratio(base_deposit_percentage, 100u128);

    // Calculate value in USD for token 0
    let value_token_0 = PrecDec::from_atomics(total_available_0, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        .checked_mul(prices.token_0_price)?;

    // Calculate value in USD for token 1
    let value_token_1 = PrecDec::from_atomics(total_available_1, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        .checked_mul(prices.token_1_price)?;

    // If the value of token0 is greater than token1, then we need to deposit more token0 depending in the `imbalance` config variable.
    // We calculate the imbalance, and then use that to determine the amount of token0 to deposit.
    let (final_amount_0, final_amount_1) = if value_token_0 > value_token_1 {
        let imbalance = (value_token_0 - value_token_1) * PrecDec::percent(config_imbalance);

        let additional_token_0 = imbalance.checked_div(prices.token_0_price)?;

        let final_0 = computed_amount_0
            + Uint128::try_from(additional_token_0.to_uint_floor())
                .map_err(|_| ContractError::ConversionError)?;
        let final_1 = computed_amount_1;
        (final_0, final_1)
    // If the value of token1 is greater than token0, then we do the same thing but in the other direction.
    } else if value_token_1 > value_token_0 {
        let imbalance = (value_token_1 - value_token_0) * PrecDec::percent(config_imbalance);

        let additional_token_1 = imbalance.checked_div(prices.token_1_price)?;

        let final_0 = computed_amount_0;
        let final_1 = computed_amount_1
            + Uint128::try_from(additional_token_1.to_uint_floor())
                .map_err(|_| ContractError::ConversionError)?;
        (final_0, final_1)
    } else {
        // if the tokens are it complete balance, return the base deposit percentages of both tokens.
        (computed_amount_0, computed_amount_1)
    };

    // If the amount of token 0 to deposit is greater than the total available, return the total available.
    let final_amount_0 = if final_amount_0 > total_available_0 {
        total_available_0
    } else {
        final_amount_0
    };

    // If the amount of token 1 to deposit is greater than the total available, return the total available.
    let final_amount_1 = if final_amount_1 > total_available_1 {
        total_available_1
    } else {
        final_amount_1
    };

    // Get the total value of the token0 in USD
    let total_value_token_0 = PrecDec::from_atomics(total_available_0, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        .checked_mul(prices.token_0_price)?;

    // Get the total value of the token1 in USD
    let total_value_token_1 = PrecDec::from_atomics(total_available_1, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        .checked_mul(prices.token_1_price)?;

    // If skew is enabled, calculate the adjusted tick index based on the value imbalance
    let adjusted_tick_index = if skew {
        calculate_adjusted_tick_index(tick_index, fee, total_value_token_0, total_value_token_1)?
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
    // If values are zero
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
    let imbalance_f64 = if value_token_0 >= value_token_1 {
        // Token0 dominates or equal - positive imbalance
        let imbalance = value_token_0
            .checked_sub(value_token_1)?
            .checked_div(total_value)?;

        imbalance
            .to_string()
            .parse::<f64>()
            .map_err(|_| ContractError::ConversionError)?
    } else {
        // Token1 dominates - negative imbalance
        let imbalance = value_token_1
            .checked_sub(value_token_0)?
            .checked_div(total_value)?;

        -imbalance
            .to_string()
            .parse::<f64>()
            .map_err(|_| ContractError::ConversionError)?
    };

    // Calculate the adjustment linearly based on the imbalance
    let adjustment = (imbalance_f64 * max_adjustment as f64).round() as i64;

    // Apply the adjustment to the base tick index
    Ok(base_tick_index + adjustment)
}

/// Extract the amounts of token0 and token1 from the withdrawal response.
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

/// Extract the denom from the create denom response.
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

/// Get the virtual contract balance. Which includes all the tokens deposited in AMM positions + the tokens available in the contract.
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

/// Get the amount of shares to mint.
/// This is used to calculate the mint amount when a user deposits tokens.
pub fn get_mint_amount(
    config: Config,
    prices: CombinedPriceResponse,
    deposited_value_token_0: PrecDec,
    deposited_value_token_1: PrecDec,
    total_amount_0: Uint128,
    total_amount_1: Uint128,
) -> Result<Uint128, ContractError> {
    let mut total_shares: PrecDec = PrecDec::zero();

    // Get the total value of the remaining tokens
    let total_value_token_0 = PrecDec::from_atomics(total_amount_0, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        .checked_mul(prices.token_0_price)?;
    let total_value_token_1 = PrecDec::from_atomics(total_amount_1, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        .checked_mul(prices.token_1_price)?;

    // get the value of the incoming deposit.
    let deposit_value_incoming = deposited_value_token_0
        .checked_add(deposited_value_token_1)
        .unwrap();
    // get the total value of the existing tokens in the contract.
    let total_value_existing = total_value_token_0
        .checked_add(total_value_token_1)?
        .checked_sub(deposit_value_incoming)?;

    if config.total_shares == Uint128::zero() {
        // Initial deposit - set shares equal to deposit value.
        // we multiply by the SHARES_MULTIPLIER which sets the standard for the share amount for future deposits.
        // having a large number of shares allows for more percision.
        total_shares =
            deposit_value_incoming.checked_mul(PrecDec::from_ratio(SHARES_MULTIPLIER, 1u128))?;
    } else {
        // Calculate proportional shares based on the ratio of deposit value to total value
        total_shares = deposit_value_incoming
            .checked_mul(PrecDec::from_ratio(config.total_shares, 1u128))
            .map_err(|_| ContractError::ConversionError)?
            .checked_div(total_value_existing)
            .map_err(|_| ContractError::ConversionError)?;
    }

    let shares_u128 = precdec_to_uint128(total_shares)?;

    if shares_u128.is_zero() {
        return Err(ContractError::InvalidTokenAmount);
    }
    Ok(shares_u128)
}

/// Convert a PrecDec to a Uint128.
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

/// Get the deposit messages.
/// This is used to get the deposit messages that will perform AM deposits with the contract funds.
pub fn get_deposit_messages(
    env: &Env,
    config: Config,
    tick_index: i64,
    prices: crate::msg::CombinedPriceResponse,
    token_0_balance: Uint128,
    token_1_balance: Uint128,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let mut messages = Vec::new();

    if config.paused {
        return Ok(messages);
    }

    // get the amount to deposit at the tightest spread
    let deposit_data = get_deposit_data(
        token_0_balance,
        token_1_balance,
        tick_index,
        config.fee_tier_config.fee_tiers[0].fee,
        &prices,
        config.fee_tier_config.fee_tiers[0].percentage,
        config.skew,
        config.imbalance,
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
                swap_on_deposit: true,
            }],
        });
        messages.push(dex_msg);
    }

    // Calculate remaining amounts after first deposit
    let remaining_amount0 = token_0_balance
        .checked_sub(deposit_data.amount0)
        .unwrap_or(Uint128::zero());
    let remaining_amount1 = token_1_balance
        .checked_sub(deposit_data.amount1)
        .unwrap_or(Uint128::zero());

    // If no remaining tokens or no additional fee tiers, return early
    if (remaining_amount0.is_zero() && remaining_amount1.is_zero())
        || config.fee_tier_config.fee_tiers.len() <= 1
    {
        return Ok(messages);
    }

    // Calculate sum of remaining percentages
    let remaining_percentages: u64 = config
        .fee_tier_config
        .fee_tiers
        .iter()
        .skip(1)
        .map(|tier| tier.percentage)
        .sum();

    if remaining_percentages == 0 {
        return Ok(messages);
    }

    // Process remaining fee tiers
    let remaining_tiers = config
        .fee_tier_config
        .fee_tiers
        .iter()
        .skip(1)
        .collect::<Vec<_>>();

    // Calculate the total amount to distribute for each token
    let total_amount0_to_distribute = remaining_amount0;
    let total_amount1_to_distribute = remaining_amount1;

    let mut distributed_amount0 = Uint128::zero();
    let mut distributed_amount1 = Uint128::zero();

    for (i, fee_tier) in remaining_tiers.iter().enumerate() {
        // For the last tier, use all remaining tokens
        if i == remaining_tiers.len() - 1 {
            let amount_0 = total_amount0_to_distribute - distributed_amount0;
            let amount_1 = total_amount1_to_distribute - distributed_amount1;

            // Skip if both amounts are zero
            if amount_0.is_zero() && amount_1.is_zero() {
                continue;
            }

            // Create deposit message
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
                    swap_on_deposit: true,
                }],
            });
            messages.push(dex_msg);
        } else {
            // Calculate exact amount based on percentage. We scale this so we get the exact amounts.
            let amount_0 = total_amount0_to_distribute
                .multiply_ratio(fee_tier.percentage as u128, remaining_percentages as u128);
            let amount_1 = total_amount1_to_distribute
                .multiply_ratio(fee_tier.percentage as u128, remaining_percentages as u128);

            // Skip if both amounts are zero
            if amount_0.is_zero() && amount_1.is_zero() {
                continue;
            }

            // Track distributed amounts
            distributed_amount0 += amount_0;
            distributed_amount1 += amount_1;

            // Create deposit message
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
                    swap_on_deposit: true,
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

/// Get the withdrawal messages.
/// This is used to burn get the message sequence for burning LP tokens and crediting
/// the beneficiary with the proportinal value of the total funds in the contract.
pub fn get_withdrawal_messages(
    env: &Env,
    deps: &DepsMut,
    config: &Config,
    burn_amount: Uint128,
    beneficiary: String,
    total_amount_0: Uint128,
    total_amount_1: Uint128,
) -> Result<(Vec<CosmosMsg>, Uint128, Uint128), ContractError> {
    let mut messages: Vec<CosmosMsg> = vec![];

    let total_supply: Uint128 = deps.querier.query_supply(&config.lp_denom)?.amount;
    if burn_amount > total_supply {
        return Err(ContractError::InvalidWithdrawAmount);
    }
    // Calculate withdrawal amounts using multiplication before division to prevent precision loss
    // and potential overflow. result is floored by default
    let withdraw_amount_0 = total_amount_0.multiply_ratio(burn_amount, total_supply);
    let withdraw_amount_1 = total_amount_1.multiply_ratio(burn_amount, total_supply);

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

    if !(withdraw_amount_0.is_zero()) {
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

    if !(withdraw_amount_1.is_zero()) {
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

/// Checks if the contract is stale and handles the pause logic.
/// Returns Ok(None) if the contract is not stale or on hold.
/// Returns Ok(Some(Response)) if the contract is stale and funds need to be returned.
/// Returns Err if the contract is on hold.
pub fn check_staleness(
    env: &Env,
    info: &MessageInfo,
    config: &mut Config,
) -> Result<Option<Response>, ContractError> {
    // get the last executed timestamp.
    let is_stale: bool = (env.block.time.seconds() - config.last_executed) > config.timestamp_stale;

    // if we are currently on hold, block all calls during this block height.
    if config.pause_block == env.block.height {
        return Err(ContractError::BlockOnHold {});
    }

    // if we are stale but not on hold, we should set the pause_block to the current block.
    // If next block comes in a timely manner (less than timestamp_stale), we should no longer be stale
    // and should be clear of the pause_block as it is only set when stale.
    if is_stale {
        config.last_executed = env.block.time.seconds();
        config.pause_block = env.block.height;

        // Return a response with the updated config and messages
        let mut messages: Vec<CosmosMsg> = vec![];
        for coin in info.funds.iter() {
            messages.push(
                BankMsg::Send {
                    to_address: info.sender.to_string(),
                    amount: vec![Coin {
                        denom: coin.denom.clone(),
                        amount: coin.amount,
                    }],
                }
                .into(),
            );
        }

        // The caller will save the config
        return Ok(Some(Response::new().add_messages(messages)));
    }

    // Update the timestamp but don't save yet
    config.last_executed = env.block.time.seconds();

    // The caller will save the config
    Ok(None)
}

/// Prepare the state for the AMM Deposit.
/// It will perform a limit order deposit at the tightest spread to ensure no liquidity is placed behind enemy lines.
/// if there is liquidity behind enemy lines, this LO will either fully clear it or exhause the token reserves of the contract in the attempt.
pub fn prepare_state(
    deps: &DepsMut,
    env: &Env,
    config: &Config,
    index: i64,
    prices: crate::msg::CombinedPriceResponse,
) -> Result<Vec<CosmosMsg>, ContractError> {
    let mut messages: Vec<CosmosMsg> = vec![];
    let target_tick_index_0 = index + config.fee_tier_config.fee_tiers[0].fee as i64;
    let target_tick_index_1 = -index + config.fee_tier_config.fee_tiers[0].fee as i64;

    let balances = query_contract_balance(deps, env.clone(), config.pair_data.clone())?;
    let token_0_usable = balances[0].amount;
    let token_1_usable = balances[1].amount;

    // First limit order simulation (token 0 -> token 1)
    let limit_order_msg_token_0 = MsgPlaceLimitOrder {
        creator: env.contract.address.to_string(),
        receiver: env.contract.address.to_string(),
        token_in: config.pair_data.token_0.denom.clone(),
        token_out: config.pair_data.token_1.denom.clone(),
        tick_index_in_to_out: target_tick_index_0,
        amount_in: token_0_usable.to_string(),
        order_type: LimitOrderType::ImmediateOrCancel.into(),
        expiration_time: None,
        max_amount_out: None,
        limit_sell_price: None,
        min_average_sell_price: Some(
            prices
                .token_0_price
                .checked_mul(PrecDec::percent(90))?
                .to_prec_dec_string(),
        ),
    };
    messages.push(limit_order_msg_token_0.into());
    // Second limit order simulation (token 1 -> token 0)
    let limit_order_msg_token_1 = MsgPlaceLimitOrder {
        creator: env.contract.address.to_string(),
        receiver: env.contract.address.to_string(),
        token_in: config.pair_data.token_1.denom.clone(),
        token_out: config.pair_data.token_0.denom.clone(),
        tick_index_in_to_out: target_tick_index_1,
        amount_in: token_1_usable.to_string(),
        order_type: LimitOrderType::ImmediateOrCancel.into(),
        expiration_time: None,
        max_amount_out: None,
        limit_sell_price: None,
        min_average_sell_price: Some(
            prices
                .token_1_price
                .checked_mul(PrecDec::percent(90))?
                .to_prec_dec_string(),
        ),
    };
    messages.push(limit_order_msg_token_1.into());
    Ok(messages)
}
