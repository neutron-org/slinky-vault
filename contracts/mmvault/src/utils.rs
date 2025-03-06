use std::str::FromStr;

use crate::error::{ContractError, ContractResult};
use crate::msg::{CombinedPriceResponse, DepositResult};
use crate::state::{Config, PairData, TokenData, CONFIG, SHARES_MULTIPLIER, WITHDRAW_REPLY_ID};
use cosmwasm_std::{
    BalanceResponse, BankQuery, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, Int128, QueryRequest,
    ReplyOn, Response, SubMsg, SubMsgResponse, Uint128, BankMsg
};
use neutron_std::types::neutron::util::precdec::PrecDec;
use neutron_std::types::osmosis::tokenfactory::v1beta1::MsgCreateDenomResponse;
use neutron_std::types::{
    neutron::dex::{
        DepositOptions, DexQuerier, MsgDeposit, MsgWithdrawal, MsgWithdrawalResponse,
        QueryAllUserDepositsResponse,
    },
    slinky::{
        marketmap::v1::{MarketMap, MarketResponse, MarketmapQuerier},
        oracle::v1::{GetAllCurrencyPairsResponse, GetPriceResponse, OracleQuerier},
        types::v1::CurrencyPair,
    },
};
use neutron_std::types::osmosis::tokenfactory::v1beta1::{MsgBurn, MsgCreateDenom, MsgMint};

use prost::Message;
use serde_json::to_vec;

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

// pub fn query_oracle_price(deps: &Deps, pair: &CurrencyPair) -> ContractResult<GetPriceResponse> {
//     let querier = OracleQuerier::new(&deps.querier);
//     let price: GetPriceResponse = querier.get_price(Some(pair.clone()))?;
//     Ok(price)
// }

// pub fn query_marketmap_market(deps: &Deps, pair: &CurrencyPair) -> ContractResult<MarketResponse> {
//     let querier = MarketmapQuerier::new(&deps.querier);
//     let market_response: MarketResponse = querier.market(Some(pair.clone()))?;
//     Ok(market_response)
// }

// pub fn query_oracle_currency_pairs(deps: &Deps) -> ContractResult<Vec<CurrencyPair>> {
//     let querier = OracleQuerier::new(&deps.querier);
//     let oracle_currency_pairs_response: GetAllCurrencyPairsResponse =
//         querier.get_all_currency_pairs()?;
//     Ok(oracle_currency_pairs_response.currency_pairs)
// }

// pub fn query_marketmap_market_map(deps: &Deps) -> ContractResult<MarketMap> {
//     let querier = MarketmapQuerier::new(&deps.querier);
//     let marketmap_currency_pairs_response = querier.market_map()?;
//     Ok(marketmap_currency_pairs_response.market_map.unwrap())
// }

// pub fn validate_market(
//     deps: &Deps,
//     env: &Env,
//     pair: &CurrencyPair,
//     max_blocks_old: u64,
// ) -> ContractResult<Response> {
//     // quote asset is USD, don't check price of USD / USD
//     if is_usd_denom(&pair.base) {
//         return Ok(Response::new());
//     }

//     // get price response here to avoid querying twice on recent and not_nil checks
//     let price_response = query_oracle_price(deps, pair)?;
//     validate_market_supported_xoracle(deps, pair, None)?;
//     validate_market_supported_xmarketmap(deps, pair, None)?;
//     //validate_market_enabled(deps, &pair, None)?;
//     validate_price_recent(
//         deps,
//         env,
//         pair,
//         max_blocks_old,
//         Some(price_response.clone()),
//     )?;
//     validate_price_not_nil(deps, pair, Some(price_response.clone()))?;
//     Ok(Response::new())
// }

// pub fn validate_price_recent(
//     deps: &Deps,
//     env: &Env,
//     pair: &CurrencyPair,
//     max_blocks_old: u64,
//     oracle_price_response: Option<GetPriceResponse>,
// ) -> ContractResult<Response> {
//     let current_block_height: u64 = env.block.height;
//     let oracle_price_response = match oracle_price_response {
//         Some(response) => response,
//         None => query_oracle_price(deps, pair)?,
//     };

//     let price: neutron_std::types::slinky::oracle::v1::QuotePrice = oracle_price_response
//         .price
//         .ok_or_else(|| ContractError::PriceNotAvailable {
//             symbol: pair.base.clone(),
//             quote: pair.quote.clone(),
//         })?;
//     if (current_block_height - price.block_height) > max_blocks_old {
//         return Err(ContractError::PriceTooOld {
//             symbol: pair.base.clone(),
//             quote: pair.quote.clone(),
//             max_blocks: max_blocks_old,
//         });
//     }

//     Ok(Response::new())
// }

// pub fn validate_market_enabled(
//     deps: &Deps,
//     pair: &CurrencyPair,
//     marketmap_market_response: Option<MarketResponse>,
// ) -> ContractResult<Response> {
//     let marketmap_market_response: MarketResponse = match marketmap_market_response {
//         Some(response) => response,
//         None => query_marketmap_market(deps, pair)?,
//     };

//     if let Some(market) = marketmap_market_response.market {
//         if let Some(ticker) = market.ticker {
//             if !ticker.enabled {
//                 return Err(ContractError::UnsupportedMarket {
//                     symbol: pair.base.clone(),
//                     quote: pair.quote.clone(),
//                     location: "x/marketmap".to_string(),
//                 });
//             }
//         }
//     }
//     Ok(Response::new())
// }

// pub fn validate_market_supported_xoracle(
//     deps: &Deps,
//     pair: &CurrencyPair,
//     oracle_currency_pairs: Option<Vec<CurrencyPair>>,
// ) -> ContractResult<Response> {
//     let supported_pairs = match oracle_currency_pairs {
//         Some(pairs) => pairs,
//         None => query_oracle_currency_pairs(deps)?,
//     };

//     if !supported_pairs.contains(pair) {
//         return Err(ContractError::UnsupportedMarket {
//             symbol: pair.base.clone(),
//             quote: pair.quote.clone(),
//             location: "x/oracle".to_string(),
//         });
//     }

//     Ok(Response::new())
// }

// pub fn validate_market_supported_xmarketmap(
//     deps: &Deps,
//     pair: &CurrencyPair,
//     market_map: Option<MarketMap>,
// ) -> ContractResult<Response> {
//     let map = match market_map {
//         Some(map) => map,
//         None => query_marketmap_market_map(deps)?,
//     };
//     let key: String = format!("{}/{}", pair.base, pair.quote);
//     if !map.markets.contains_key(&key) {
//         return Err(ContractError::UnsupportedMarket {
//             symbol: pair.base.clone(),
//             quote: pair.quote.clone(),
//             location: "x/marketmap".to_string(),
//         });
//     }

//     Ok(Response::new())
// }

// pub fn validate_price_not_nil(
//     deps: &Deps,
//     pair: &CurrencyPair,
//     oracle_price_response: Option<GetPriceResponse>,
// ) -> ContractResult<Response> {
//     let oracle_price_response = match oracle_price_response {
//         Some(response) => response,
//         None => query_oracle_price(deps, pair)?,
//     };

//     if oracle_price_response.nonce == 0 {
//         return Err(ContractError::PriceIsNil {
//             symbol: pair.base.clone(),
//             quote: pair.quote.clone(),
//         });
//     }
//     Ok(Response::new())
// }

// pub fn get_prices(deps: Deps, env: Env) -> ContractResult<CombinedPriceResponse> {
//     let config = CONFIG.load(deps.storage)?;

//     // Helper function to get price or return 1 if the base is a USD denom
//     fn get_price_or_default(
//         deps: &Deps,
//         env: &Env,
//         pair: &CurrencyPair,
//         max_blocks_old: u64,
//     ) -> ContractResult<PrecDec> {
//         // Check if the pair's base is USD denom
//         if is_usd_denom(&pair.base) {
//             return Ok(PrecDec::one());
//         }

//         // Query the oracle for the price
//         let price_response = query_oracle_price(deps, pair)?;
//         validate_price_not_nil(deps, pair, Some(price_response.clone()))?;
//         validate_price_recent(
//             deps,
//             env,
//             pair,
//             max_blocks_old,
//             Some(price_response.clone()),
//         )?;

//         // Parse the price string to Int128 and normalize
//         let price_int128 = Int128::from_str(&price_response.price.unwrap().price)
//             .map_err(|_| ContractError::InvalidPrice)?;
//         let price = normalize_price(price_int128, price_response.decimals)?;

//         Ok(price)
//     }

//     // Get prices for token_0 and token_1, or default to 1 for valid currencies
//     let pair_1 = config.pair_data.token_0.pair;
//     let token_0_price =
//         get_price_or_default(&deps, &env, &pair_1, config.max_blocks_old)?.checked_div(
//             PrecDec::from_ratio(10u128.pow(config.pair_data.token_0.decimals.into()), 1u128),
//         )?;

//     let pair_2 = config.pair_data.token_1.pair;
//     let token_1_price =
//         get_price_or_default(&deps, &env, &pair_2, config.max_blocks_old)?.checked_div(
//             PrecDec::from_ratio(10u128.pow(config.pair_data.token_1.decimals.into()), 1u128),
//         )?;

//     // Calculate the price ratio
//     let price_0_to_1 = price_ratio(token_0_price, token_1_price);
//     let res = CombinedPriceResponse {
//         token_0_price,
//         token_1_price,
//         price_0_to_1,
//     };

//     Ok(res)
// }

// pub fn normalize_price(price: Int128, decimals: u64) -> ContractResult<PrecDec> {
//     // Ensure decimals does not exceed u32::MAX
//     if decimals > u32::MAX as u64 {
//         return Err(ContractError::TooManyDecimals);
//     }
//     if price < Int128::zero() {
//         return Err(ContractError::PriceIsNegative);
//     }
//     let abs_value: u128 = price.i128().unsigned_abs();
//     PrecDec::from_atomics(abs_value, decimals as u32)
//         .map_err(|_e| ContractError::DecimalConversionError)
// }

// fn price_ratio(price_1: PrecDec, price_2: PrecDec) -> PrecDec {
//     price_1 / price_2
// }

// pub fn is_usd_denom(currency: &str) -> bool {
//     matches!(currency, "USD" | "USDC")
// }

// pub fn uint128_to_int128(u: Uint128) -> Result<Int128, ContractError> {
//     let value = u.u128();
//     if value > i128::MAX as u128 {
//         return Err(ContractError::ConversionError);
//     }
//     Ok(Int128::from(value as i128))
// }

// pub fn int128_to_uint128(i: Int128) -> Result<Uint128, ContractError> {
//     let value = i.i128();
//     if value < 0 {
//         return Err(ContractError::ConversionError);
//     }
//     Ok(Uint128::from(value as u128))
// }
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

/// Updates the balances in the provided config object.
pub fn update_contract_balance(
    deps: &DepsMut,
    env: Env,
    config: &mut Config,
) -> Result<(), ContractError> {
    // Query the contract balances for the two tokens
    let balances = query_contract_balance(deps, env, config.pair_data.clone())?;

    // Update the config balances based on the queried balances
    config.balances.token_0.amount = balances[0].amount;
    config.balances.token_1.amount = balances[1].amount;

    Ok(())
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
) -> Result<DepositResult, ContractError> {
    // Calculate the base deposit amounts
    let computed_amount_0 = total_available_0.multiply_ratio(base_deposit_percentage, 100u128);
    let computed_amount_1 = total_available_1.multiply_ratio(base_deposit_percentage, 100u128);

    // Calculate value in USD for token 0
    let value_token_0 = PrecDec::from_atomics(total_available_0 - computed_amount_0, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        * prices.token_0_price;

    // Calculate value in USD for token 1
    let value_token_1 = PrecDec::from_atomics(total_available_1 - computed_amount_1, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        * prices.token_1_price;

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
        let imbalance = (value_token_1 - value_token_0) * PrecDec::percent(50);
        let additional_token_1 = imbalance / prices.token_1_price;
        (
            computed_amount_0,
            computed_amount_1
                + Uint128::try_from(additional_token_1.to_uint_floor())
                    .map_err(|_| ContractError::ConversionError)?,
        )
    } else {
        (computed_amount_0, computed_amount_1)
    };

    // Prevent dust and ensure we don't exceed available amounts
    let final_amount_0 = if final_amount_0 < Uint128::new(10) {
        Uint128::zero()
    } else if final_amount_0 > total_available_0 {
        total_available_0
    } else {
        final_amount_0
    };
    let final_amount_1 = if final_amount_1 < Uint128::new(10) {
        Uint128::zero()
    } else if final_amount_1 > total_available_1 {
        total_available_1
    } else {
        final_amount_1
    };

    let result = DepositResult {
        amount0: final_amount_0,
        amount1: final_amount_1,
        tick_index,
        fee,
    };
    Ok(result)
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
pub fn get_deposited_token_amounts(
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
    deposit_amount_0: Uint128,
    deposit_amount_1: Uint128,
) -> Result<Uint128, ContractError> {
    deps.api.debug(&format!(
        ">>>>>Starting get_mint_amount with deposits: token0={}, token1={}",
        deposit_amount_0, deposit_amount_1
    ));

    let mut total_shares = PrecDec::zero();
    let balances = query_contract_balance(deps, env.clone(), config.pair_data.clone())?;
    
    deps.api.debug(&format!(
        ">>>>>Current balances: token0={}, token1={}",
        balances[0].amount, balances[1].amount
    ));

    let prices = get_prices(deps.as_ref(), env.clone())?;
    deps.api.debug(&format!(
        ">>>>>Current prices: token0_price={}, token1_price={}",
        prices.token_0_price, prices.token_1_price
    ));

    let (deposited_amount_0, deposited_amount_1) =
        get_deposited_token_amounts(env.clone(), deps, config.clone())?;
    deps.api.debug(&format!(
        ">>>>>Already deposited amounts: token0={}, token1={}",
        deposited_amount_0, deposited_amount_1
    ));

    let total_amount_0 = balances[0].amount;
    let total_amount_1 = balances[1].amount;
    deps.api.debug(&format!(
        ">>>>>Total amounts: token0={}, token1={}",
        total_amount_0, total_amount_1
    ));

    // Get the total value of the remaining tokens
    let total_value_token_0 = PrecDec::from_ratio(
        total_amount_0,
        10u128.pow(config.pair_data.token_0.decimals.into()),
    ) * prices.token_0_price;
    let total_value_token_1 = PrecDec::from_ratio(
        total_amount_1,
        10u128.pow(config.pair_data.token_1.decimals.into()),
    ) * prices.token_1_price;

    deps.api.debug(&format!(
        ">>>>>Total values: token0_value={}, token1_value={}",
        total_value_token_0, total_value_token_1
    ));

    // Get the total value of the incoming tokens
    let deposited_value_token_0 = PrecDec::from_atomics(deposit_amount_0, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        * prices.token_0_price;
    let deposited_value_token_1 = PrecDec::from_atomics(deposit_amount_1, 0)
        .map_err(|_| ContractError::DecimalConversionError)?
        * prices.token_1_price;

    deps.api.debug(&format!(
        ">>>>>Deposited values: token0_value={}, token1_value={}",
        deposited_value_token_0, deposited_value_token_1
    ));

    let total_value_combined = total_value_token_0 + total_value_token_1;
    let deposit_value_incoming = deposited_value_token_0
        .checked_add(deposited_value_token_1)
        .unwrap();

    deps.api.debug(&format!(
        ">>>>>Total combined value={}, Incoming deposit value={}",
        total_value_combined, deposit_value_incoming
    ));

    if config.total_shares == Uint128::zero() {
        // Initial deposit - set shares equal to deposit value
        deps.api.debug(">>>>>First deposit - using deposit value as shares");
        total_shares = deposit_value_incoming;
    } else {
        deps.api.debug(&format!(">>>>>Existing shares: {}", config.total_shares));
        // Calculate proportional shares based on the ratio of deposit value to total value
        total_shares = deposit_value_incoming
            .checked_mul(PrecDec::from_ratio(config.total_shares, 1u128))
            .map_err(|_| ContractError::ConversionError)?
            .checked_div(total_value_combined)
            .map_err(|_| ContractError::ConversionError)?;
    }

    deps.api.debug(&format!(">>>>>Calculated total_shares: {}", total_shares));

    let shares_returned = precdec_to_uint128(
        total_shares
            .checked_mul(PrecDec::from_ratio(SHARES_MULTIPLIER, 1u128))
            .map_err(|_| ContractError::ConversionError)?,
    )?;

    deps.api.debug(&format!(">>>>>Final shares_returned: {}", shares_returned));

    if shares_returned.is_zero() {
        return Err(ContractError::InvalidTokenAmount);
    }

    Ok(shares_returned)
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

    // Calculate remaining amounts for ambient deposit
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
    deps: &DepsMut,
    messages: &[Vec<CosmosMsg>],
    reply_id: u64,
    payload: Option<Binary>,
) -> Result<Vec<SubMsg>, ContractError> {
    let mut submsgs: Vec<SubMsg> = messages.concat().into_iter().map(SubMsg::new).collect();

    deps.api.debug(&format!(">>>>>>flatten_msgs_always_reply: messages={:?}", messages));

    if submsgs.is_empty() {
        deps.api.debug(">>>>>>flatten_msgs_always_reply: No messages to process");
        return Err(ContractError::NoFundsAvailable {});
    }

    // Add reply to the last message
    if let Some(last) = submsgs.last_mut() {
        deps.api.debug(&format!(">>>>>>flatten_msgs_always_reply: Adding reply to last message: {:?}", last));
        last.id = reply_id;
        last.reply_on = ReplyOn::Success;
        last.payload = payload.unwrap_or_default();
    }

    deps.api.debug(&format!(">>>>>>flatten_msgs_always_reply: final submsgs={:?}", submsgs));
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

    let balances = query_contract_balance(&deps, env.clone(), config.pair_data.clone())?;
    let total_supply: Uint128 = deps.querier.query_supply(&config.lp_denom)?.amount;
    // Calculate withdrawal amounts using multiplication before division to prevent precision loss
    // and potential overflow
    let withdraw_amount_0 = balances[0].amount.multiply_ratio(burn_amount, total_supply);
    let withdraw_amount_1 = balances[1].amount.multiply_ratio(burn_amount, total_supply);
    deps.api.debug(&format!(">>>>>>Withdrawal amounts: token0={}, token1={}", withdraw_amount_0, withdraw_amount_1));
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

    if !(config.balances.token_0.amount.is_zero() && withdraw_amount_0.is_zero()) {
        messages.push(
            BankMsg::Send {
                to_address: beneficiary.clone(),
                amount: vec![Coin {
                    denom: config.balances.token_0.denom.clone(),
                    amount: withdraw_amount_0,
                }],
            }
            .into(),
        );
    }

    if !(config.balances.token_1.amount.is_zero() && withdraw_amount_1.is_zero()) {
        messages.push(
            BankMsg::Send {
                to_address: beneficiary.clone(),
                amount: vec![Coin {
                    denom: config.balances.token_1.denom.clone(),
                    amount: withdraw_amount_1,
                }],
            }
            .into(),
        );
    }
    Ok((messages, withdraw_amount_0, withdraw_amount_1))
}