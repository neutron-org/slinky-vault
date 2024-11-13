use std::str::FromStr;

use crate::error::{ContractError, ContractResult};
use crate::msg::{
CombinedPriceResponse, DepositResult};
use crate::state::{Config, PairData, TokenData, CONFIG};
use cosmwasm_std::{
    BalanceResponse, BankQuery, Coin, CosmosMsg,
    Decimal, Deps, DepsMut, Env, Int128, QueryRequest, Response,
    Uint128
};
use neutron_std::types::{
    cosmos::base::query::v1beta1::PageRequest,
    neutron::dex::{
        DexQuerier, LimitOrderType, MsgPlaceLimitOrder, TickLiquidity,
        tick_liquidity::Liquidity,
    },
    slinky::{
        marketmap::v1::{MarketMap, MarketResponse, MarketmapQuerier},
        oracle::v1::{GetAllCurrencyPairsResponse, GetPriceResponse, OracleQuerier},
        types::v1::CurrencyPair,
    },
};

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

pub fn query_oracle_price(deps: &Deps, pair: &CurrencyPair) -> ContractResult<GetPriceResponse> {
    let querier = OracleQuerier::new(&deps.querier);
    let price: GetPriceResponse = querier.get_price(Some(pair.clone()))?;
    Ok(price)
}

pub fn query_marketmap_market(deps: &Deps, pair: &CurrencyPair) -> ContractResult<MarketResponse> {
    let querier = MarketmapQuerier::new(&deps.querier);
    let market_response: MarketResponse = querier.market(Some(pair.clone()))?;
    Ok(market_response)
}

pub fn query_oracle_currency_pairs(deps: &Deps) -> ContractResult<Vec<CurrencyPair>> {
    let querier = OracleQuerier::new(&deps.querier);
    let oracle_currency_pairs_response: GetAllCurrencyPairsResponse =
        querier.get_all_currency_pairs()?;
    Ok(oracle_currency_pairs_response.currency_pairs)
}

pub fn query_marketmap_market_map(deps: &Deps) -> ContractResult<MarketMap> {
    let querier = MarketmapQuerier::new(&deps.querier);
    let marketmap_currency_pairs_response = querier.market_map()?;
    Ok(marketmap_currency_pairs_response.market_map.unwrap())
}

pub fn validate_market(
    deps: &Deps,
    env: &Env,
    pair: &CurrencyPair,
    max_blocks_old: u64,
) -> ContractResult<Response> {
    // quote asset is USD, don't check price of USD / USD
    if is_usd_denom(&pair.base) {
        return Ok(Response::new());
    }

    // get price response here to avoid querying twice on recent and not_nil checks
    let price_response = query_oracle_price(deps, pair)?;
    validate_market_supported_xoracle(deps, &pair, None)?;
    validate_market_supported_xmarketmap(deps, &pair, None)?;
    //validate_market_enabled(deps, &pair, None)?;
    validate_price_recent(
        deps,
        env,
        &pair,
        max_blocks_old,
        Some(price_response.clone()),
    )?;
    validate_price_not_nil(deps, &pair, Some(price_response.clone()))?;
    Ok(Response::new())
}

pub fn validate_price_recent(
    deps: &Deps,
    env: &Env,
    pair: &CurrencyPair,
    max_blocks_old: u64,
    oracle_price_response: Option<GetPriceResponse>,
) -> ContractResult<Response> {
    let current_block_height: u64 = env.block.height;
    let oracle_price_response = match oracle_price_response {
        Some(response) => response,
        None => query_oracle_price(deps, &pair)?,
    };

    let price: neutron_std::types::slinky::oracle::v1::QuotePrice = oracle_price_response
        .price
        .ok_or_else(|| ContractError::PriceNotAvailable {
            symbol: pair.base.clone(),
            quote: pair.quote.clone(),
        })?;
    if (current_block_height - price.block_height) > max_blocks_old {
        return Err(ContractError::PriceTooOld {
            symbol: pair.base.clone(),
            quote: pair.quote.clone(),
            max_blocks: max_blocks_old,
        });
    }

    Ok(Response::new())
}

pub fn validate_market_enabled(
    deps: &Deps,
    pair: &CurrencyPair,
    marketmap_market_response: Option<MarketResponse>,
) -> ContractResult<Response> {
    let marketmap_market_response: MarketResponse = match marketmap_market_response {
        Some(response) => response,
        None => query_marketmap_market(deps, &pair)?,
    };

    if let Some(market) = marketmap_market_response.market {
        if let Some(ticker) = market.ticker {
            if !ticker.enabled {
                return Err(ContractError::UnsupportedMarket {
                    symbol: pair.base.clone(),
                    quote: pair.quote.clone(),
                    location: "x/marketmap".to_string(),
                });
            }
        }
    }
    Ok(Response::new())
}

pub fn validate_market_supported_xoracle(
    deps: &Deps,
    pair: &CurrencyPair,
    oracle_currency_pairs: Option<Vec<CurrencyPair>>,
) -> ContractResult<Response> {
    let supported_pairs = match oracle_currency_pairs {
        Some(pairs) => pairs,
        None => query_oracle_currency_pairs(deps)?,
    };

    if !supported_pairs.contains(pair) {
        return Err(ContractError::UnsupportedMarket {
            symbol: pair.base.clone(),
            quote: pair.quote.clone(),
            location: "x/oracle".to_string(),
        });
    }

    Ok(Response::new())
}

pub fn validate_market_supported_xmarketmap(
    deps: &Deps,
    pair: &CurrencyPair,
    market_map: Option<MarketMap>,
) -> ContractResult<Response> {
    let map = match market_map {
        Some(map) => map,
        None => query_marketmap_market_map(deps)?,
    };
    let key: String = format!("{}/{}", pair.base, pair.quote);
    if map.markets.contains_key(&key) == false {
        return Err(ContractError::UnsupportedMarket {
            symbol: pair.base.clone(),
            quote: pair.quote.clone(),
            location: "x/marketmap".to_string(),
        });
    }

    Ok(Response::new())
}

pub fn validate_price_not_nil(
    deps: &Deps,
    pair: &CurrencyPair,
    oracle_price_response: Option<GetPriceResponse>,
) -> ContractResult<Response> {
    let oracle_price_response = match oracle_price_response {
        Some(response) => response,
        None => query_oracle_price(deps, &pair)?,
    };

    if oracle_price_response.nonce == 0 {
        return Err(ContractError::PriceIsNil {
            symbol: pair.base.clone(),
            quote: pair.quote.clone(),
        });
    }
    Ok(Response::new())
}

pub fn get_prices(deps: Deps, env: Env) -> ContractResult<CombinedPriceResponse> {
    let config = CONFIG.load(deps.storage)?;

    // Helper function to get price or return 1 if the base is a USD denom
    fn get_price_or_default(
        deps: &Deps,
        env: &Env,
        pair: &CurrencyPair,
        max_blocks_old: u64,
    ) -> ContractResult<Decimal> {
        // Check if the pair's base is USD denom
        if is_usd_denom(&pair.base) {
            return Ok(Decimal::one());
        }

        // Query the oracle for the price
        let price_response = query_oracle_price(deps, pair)?;
        validate_price_not_nil(deps, pair, Some(price_response.clone()))?;
        validate_price_recent(
            deps,
            env,
            pair,
            max_blocks_old,
            Some(price_response.clone()),
        )?;

        // Parse the price string to Int128 and normalize
        let price_int128 = Int128::from_str(&price_response.price.unwrap().price)
            .map_err(|_| ContractError::InvalidPrice)?;
        let price = normalize_price(price_int128, price_response.decimals)?;

        Ok(price)
    }

    // Get prices for token_0 and token_1, or default to 1 for valid currencies
    let pair_1 = config.pair_data.token_0.pair;
    let token_0_price = get_price_or_default(&deps, &env, &pair_1, config.max_blocks_old)?;

    let pair_2 = config.pair_data.token_1.pair;
    let token_1_price = get_price_or_default(&deps, &env, &pair_2, config.max_blocks_old)?;

    // Calculate the price ratio
    let price_0_to_1 = price_ratio(token_0_price, token_1_price);
    let res = CombinedPriceResponse {
        token_0_price,
        token_1_price,
        price_0_to_1,
    };

    Ok(res)
}

pub fn normalize_price(price: Int128, decimals: u64) -> ContractResult<Decimal> {
    // Ensure decimals does not exceed u32::MAX
    if decimals > u32::MAX as u64 {
        return Err(ContractError::TooManyDecimals);
    }
    if price < Int128::zero() {
        return Err(ContractError::PriceIsNegative);
    }
    let abs_value: u128 = price.i128().abs() as u128;
    Decimal::from_atomics(abs_value, decimals as u32)
        .map_err(|_e| ContractError::DecimalConversionError)
}

fn price_ratio(price_1: Decimal, price_2: Decimal) -> Decimal {
    price_1 / price_2
}

pub fn is_usd_denom(currency: &str) -> bool {
    match currency {
        "USD" | "USDC" => true,
        _ => false,
    }
}

pub fn uint128_to_int128(u: Uint128) -> Result<Int128, ContractError> {
    let value = u.u128();
    if value > i128::MAX as u128 {
        return Err(ContractError::ConversionError);
    }
    Ok(Int128::from(value as i128))
}

pub fn int128_to_uint128(i: Int128) -> Result<Uint128, ContractError> {
    let value = i.i128();
    if value < 0 {
        return Err(ContractError::ConversionError);
    }
    Ok(Uint128::from(value as u128))
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
    let balances = query_contract_balance(&deps, env, config.pair_data.clone())?;

    // Update the config balances based on the queried balances
    config.balances.token_0.amount = balances[0].amount;
    config.balances.token_1.amount = balances[1].amount;

    Ok(())
}

pub fn price_to_tick_index(price: Decimal) -> Result<i64, ContractError> {
    // Ensure the price is greater than 0
    if price.is_zero() || price < Decimal::zero() {
        return Err(ContractError::InvalidPrice);
    }

    // Convert Decimal to f64 by dividing the atomic value by the scaling factor
    let price_f64 = price.atomics().u128() as f64 / 10u128.pow(18) as f64; // 18 is the precision of Decimal

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
    expected_amount_0: Uint128,
    expected_amount_1: Uint128,
    tick_index: i64,
    fee: u64,
    prices: &CombinedPriceResponse,
    base_deposit_percentage: u64,
) -> Result<DepositResult, ContractError> {
    // Calculate the base deposit amounts
    let virtual_total_0 = total_available_0 + expected_amount_0;
    let virtual_total_1 = total_available_1 + expected_amount_1;
    let computed_amount_0 = virtual_total_0.multiply_ratio(base_deposit_percentage, 100u128);
    let computed_amount_1 = virtual_total_1.multiply_ratio(base_deposit_percentage, 100u128);

    // Get the total value of the remaining tokens
    let value_token_0 =
        Decimal::from_ratio(virtual_total_0 - computed_amount_0, 1u128) * prices.token_0_price;
    let value_token_1 =
        Decimal::from_ratio(virtual_total_1 - computed_amount_1, 1u128) * prices.token_1_price;

    let (final_amount_0, final_amount_1) = if value_token_0 > value_token_1 {
        let imbalance = (value_token_0 - value_token_1) * Decimal::percent(50);
        let additional_token_0 = imbalance / prices.token_0_price;
        (
            computed_amount_0
                + Uint128::try_from(additional_token_0.to_uint_floor())
                    .map_err(|_| ContractError::ConversionError)?,
            computed_amount_1,
        )
    } else if value_token_1 > value_token_0 {
        let imbalance = (value_token_1 - value_token_0) * Decimal::percent(50);
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

    Ok(DepositResult {
        amount0: final_amount_0,
        amount1: final_amount_1,
        tick_index,
        fee,
    })
}

pub fn prepare_state(
    deps: &DepsMut,
    env: &Env,
    config: &mut Config,
    prices: &CombinedPriceResponse,
    index: i64,
) -> Result<(Vec<CosmosMsg>, Uint128, Uint128), ContractError> {
    let mut messages: Vec<CosmosMsg> = vec![];
    let target_tick_index_1 = index + config.base_fee as i64;
    let target_tick_index_0 = (index * -1) + config.base_fee as i64;
    let dex_querier = DexQuerier::new(&deps.querier);

    let mut token_0_swappable: bool = false;
    let mut token_1_swappable: bool = false;

    // querry token 0 liquidity
    let token_0_liquidity_response = dex_querier.tick_liquidity_all(
        config.pair_data.pair_id.clone(),
        config.pair_data.token_0.denom.clone(),
        Some(PageRequest {
            key: Vec::new(),
            limit: 1,
            reverse: false,
            count_total: false,
            offset: 0,
        }),
    )?;

    let token_1_liquidity_response: neutron_std::types::neutron::dex::QueryAllTickLiquidityResponse = dex_querier.tick_liquidity_all(
        config.pair_data.pair_id.clone(),
        config.pair_data.token_1.denom.clone(),
        Some(PageRequest {
            key: Vec::new(),
            limit: 1,
            reverse: false,
            count_total: false,
            offset: 0,
        }),
    )?;

    // get price of cheapest token0 liquidity
    if !token_1_liquidity_response.tick_liquidity.is_empty() {
        let liq: &TickLiquidity = &token_1_liquidity_response.tick_liquidity[0];
        let lowest_tick_index_1: i64;
        let price_at_tick: Decimal;
        // Handle empty case
        match &liq.liquidity {
            Some(Liquidity::PoolReserves(reserves)) => {
                lowest_tick_index_1 = reserves.key.as_ref().map_or_else(
                    || Err(ContractError::TickIndexDoesNotExist),
                    |key| Ok(key.tick_index_taker_to_maker),
                )?;
                price_at_tick = Decimal::from_str(&reserves.price_taker_to_maker).unwrap();
            }
            Some(Liquidity::LimitOrderTranche(tranche)) => {
                lowest_tick_index_1 = tranche.key.as_ref().map_or_else(
                    || Err(ContractError::TickIndexDoesNotExist),
                    |key| Ok(key.tick_index_taker_to_maker),
                )?;
                price_at_tick = Decimal::from_str(&tranche.price_taker_to_maker).unwrap();
            }
            None => {
                return Err(ContractError::LiquidityNotFound);
            }
        }
        // trying to place 10 token0 liquidity at tick_index_0, so trying to buy token1 at tick_index_0
        // ASSUME LIQUIDITY EXISTS: 100 USDC and 100 NTRN at given ticks
        // center tick = -10711
        // lower Tick token0 (USDC in pool) Index:10731   Price:0.341965  10 NTRN * 0.341965 = 3.41965 USDC || 100USDC / 0.341965 = 292.427588 NTRN worth of USDC
        // upper tick token1 (NTRN in pool) Index:-10691  Price:2.9126    10 USDC * 2.9126   = 29.126 NTRN  || 100 NTRN / 2.9126 = 34.3333 USDC worth of NTRN
        // token_0_liquidity_response = 10731
        // token_1_liquidity_response = -10691
        // assume tick_index_0 = 10631 -> Price = 0.345402 -> 10 NTRN * 0.345402 = 3.45402 USDC || 10USDC / 0.345402 = 28.951 NTRN worth of USDC
        // assume tick_index_0 = 10691 -> Price = 0.343336 -> 10 NTRN * 0.34333571534 = 3.4333571534 USDC || 10USDC / 0.34333571534 = 29.126 NTRN worth of USDC

        // if I try to swap the USDC directly into NTRN using the available liquidity, I'll get:
        // Price: 2.9126 -> 10 * 2.9126 = 29.126 NTRN.
        // since 28.951 < 29.126, It would be chepaer to swap at current price than place liquidity : B.E.L.
        // if target_deposit_tick_index_0 < tick_index * -1
        if target_tick_index_0 <= lowest_tick_index_1 * -1 {
            token_0_swappable = true;
        }
    }
    // get tick index of cheapest token0 liquidity
    if !token_0_liquidity_response.tick_liquidity.is_empty() {
        let liq: &TickLiquidity = &token_0_liquidity_response.tick_liquidity[0];

        let lowest_tick_index_0: i64;
        // Handle empty case
        match &liq.liquidity {
            Some(Liquidity::PoolReserves(reserves)) => {
                lowest_tick_index_0 = reserves.key.as_ref().map_or_else(
                    || Err(ContractError::TickIndexDoesNotExist),
                    |key| Ok(key.tick_index_taker_to_maker),
                )?;
            }
            Some(Liquidity::LimitOrderTranche(tranche)) => {
                lowest_tick_index_0 = tranche.key.as_ref().map_or_else(
                    || Err(ContractError::TickIndexDoesNotExist),
                    |key| Ok(key.tick_index_taker_to_maker),
                )?;
            }
            None => {
                return Err(ContractError::LiquidityNotFound);
            }
        }

        if target_tick_index_1 <= lowest_tick_index_0 * -1 {
            token_1_swappable = true;
        }
    }

    let mut swapped_amount_0: Uint128 = Uint128::zero();
    let mut gained_amount_0: Uint128 = Uint128::zero();
    let mut swapped_amount_1: Uint128 = Uint128::zero();
    let mut gained_amount_1: Uint128 = Uint128::zero();

  

    if (token_0_swappable) {
        let msg: MsgPlaceLimitOrder = MsgPlaceLimitOrder {
            creator: env.contract.address.to_string(),
            min_average_sell_price: None, 
            receiver: env.contract.address.to_string(),
            token_in: config.pair_data.token_0.denom.clone(),
            token_out: config.pair_data.token_1.denom.clone(),
            tick_index_in_to_out: target_tick_index_0 + 2,
            amount_in: swapped_amount_0.to_string(),
            order_type: LimitOrderType::ImmediateOrCancel.into(),
            expiration_time: None,
            max_amount_out: None,
            // TODO: make this an option
            limit_sell_price: None
        };
        // TODO: ensure autoswap is enabled and responce includes the total pricing.
        let token_0_swap_simulation_result = dex_querier.simulate_place_limit_order(Some(msg))?;

        // get the amount of token 0 the limit order immediately used
        swapped_amount_0 = token_0_swap_simulation_result.clone()
            .resp
            .and_then(|resp| resp.taker_coin_in)
            .and_then(|coin| Uint128::from_str(&coin.amount).ok())
            .unwrap_or(Uint128::zero());
        // get the amount of token1 that the limit order immediatly produced
        // Q: Does this correctly account for auto-swap pricing? i.e autoswapping through multiple orders
        gained_amount_1 = token_0_swap_simulation_result
            .resp
            .and_then(|resp| resp.taker_coin_out)
            .and_then(|coin| Uint128::from_str(&coin.amount).ok())
            .unwrap_or(Uint128::zero());
    }

    if (token_1_swappable) {

        let msg: MsgPlaceLimitOrder = MsgPlaceLimitOrder {
            creator: env.contract.address.to_string(),
            min_average_sell_price: None, 
            receiver: env.contract.address.to_string(),
            token_in: config.pair_data.token_1.denom.clone(),
            token_out: config.pair_data.token_0.denom.clone(),
            tick_index_in_to_out: target_tick_index_0 + 2,
            amount_in: swapped_amount_1.to_string(),
            order_type: LimitOrderType::ImmediateOrCancel.into(),
            expiration_time: None,
            max_amount_out: None,
            // TODO: make this an option
            limit_sell_price: None
        };
        // TODO: ensure autoswap is enabled and responce includes the total pricing.
        let token_1_swap_simulation_result = dex_querier.simulate_place_limit_order(Some(msg))?;

        // get the amount of token 1 the limit order immediately used
        swapped_amount_1 = token_1_swap_simulation_result.clone()
            .resp
            .and_then(|resp| resp.taker_coin_in)
            .and_then(|coin| Uint128::from_str(&coin.amount).ok())
            .unwrap_or(Uint128::zero());
        // get the amount of token 0 that the limit order immediately produced
        gained_amount_0 = token_1_swap_simulation_result
            .resp
            .and_then(|resp| resp.taker_coin_out)
            .and_then(|coin| Uint128::from_str(&coin.amount).ok())
            .unwrap_or(Uint128::zero());
    }

    // let limit_sell_price_0: Decimal = prices.token_0_price * Decimal::percent(120);
    // let limit_sell_price_1: Decimal = prices.token_1_price * Decimal::percent(120);

    if swapped_amount_0 > Uint128::zero() {
        let limit_order_msg = Into::<CosmosMsg>::into(MsgPlaceLimitOrder {
            creator: env.contract.address.to_string(),
            min_average_sell_price: None, 
            receiver: env.contract.address.to_string(),
            token_in: config.pair_data.token_0.denom.clone(),
            token_out: config.pair_data.token_1.denom.clone(),
            tick_index_in_to_out: target_tick_index_0 + 2,
            amount_in: swapped_amount_0.to_string(),
            order_type: LimitOrderType::ImmediateOrCancel.into(),
            expiration_time: None,
            max_amount_out: None,
            // TODO: make this an option
            limit_sell_price: None
        });

        messages.push(limit_order_msg);

        config.balances.token_0.amount = config
            .balances
            .token_0
            .amount
            .checked_sub(swapped_amount_0)
            .unwrap_or_default();
    }
    if swapped_amount_1 > Uint128::zero() {
        let limit_order_msg = Into::<CosmosMsg>::into(MsgPlaceLimitOrder {
            creator: env.contract.address.to_string(),
            min_average_sell_price: None, 
            receiver: env.contract.address.to_string(),
            token_in: config.pair_data.token_1.denom.clone(),
            token_out: config.pair_data.token_0.denom.clone(),
            tick_index_in_to_out: target_tick_index_1 + 2,
            amount_in: swapped_amount_1.to_string(),
            order_type: LimitOrderType::ImmediateOrCancel.into(),
            expiration_time: None,
            max_amount_out: None,
            // TODO: make this an option
            limit_sell_price: None
        });

        messages.push(limit_order_msg);

        config.balances.token_1.amount = config
            .balances
            .token_1
            .amount
            .checked_sub(swapped_amount_1)
            .unwrap_or_default();
    }
    Ok((messages, gained_amount_0, gained_amount_1))
}
