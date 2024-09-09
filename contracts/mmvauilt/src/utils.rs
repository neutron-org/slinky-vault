use std::str::FromStr;

use crate::error::ContractError;
use crate::msg::{
    CombinedPriceResponse, DepositResult, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};
use crate::state::{Config, PairData, TokenData, CONFIG};
use cosmwasm_std::{
    attr, entry_point, to_json_binary, BalanceResponse, BankQuery, Binary, Coin, CosmosMsg,
    Decimal, Deps, DepsMut, Env, Fraction, Int128, MessageInfo, QueryRequest, Response, StdResult,
    Uint128, Uint64,
};
use cw2::set_contract_version;
use serde::{Deserialize, Serialize};

pub type ContractResult<T> = core::result::Result<T, ContractError>;
use neutron_sdk::bindings::marketmap::query::{MarketMapQuery, MarketMapResponse, MarketResponse};
use neutron_sdk::bindings::marketmap::types::MarketMap;

use neutron_sdk::bindings::oracle::query::{
    GetAllCurrencyPairsResponse, GetPriceResponse, GetPricesResponse, OracleQuery,
};
use neutron_sdk::proto_types::cosmos::base::query::v1beta1::PageRequest;

use neutron_sdk::bindings::dex::msg::DexMsg;
use neutron_sdk::bindings::dex::types::LimitOrderType;
use neutron_sdk::proto_types::neutron::dex;
use neutron_sdk::proto_types::neutron::dex::tick_liquidity;
use neutron_sdk::proto_types::neutron::dex::LimitOrderTranche;
use neutron_sdk::proto_types::neutron::dex::PoolReserves;
use neutron_sdk::proto_types::neutron::dex::TickLiquidity;

use neutron_sdk::bindings::oracle::types::CurrencyPair;
use neutron_sdk::bindings::{msg::NeutronMsg, query::NeutronQuery};

pub fn load_config(deps: Deps<NeutronQuery>) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

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

pub fn query_oracle_price(
    deps: &Deps<NeutronQuery>,
    pair: &CurrencyPair,
) -> ContractResult<GetPriceResponse> {
    let oracle_price_query: OracleQuery = OracleQuery::GetPrice {
        currency_pair: pair.clone(),
    };
    let oracle_price_response: GetPriceResponse = deps.querier.query(&oracle_price_query.into())?;
    Ok(oracle_price_response)
}

pub fn query_marketmap_market(
    deps: &Deps<NeutronQuery>,
    pair: &CurrencyPair,
) -> ContractResult<MarketResponse> {
    let marketmap_market_query: MarketMapQuery = MarketMapQuery::Market {
        currency_pair: pair.clone(),
    };
    let marketmap_market_response: MarketResponse =
        deps.querier.query(&marketmap_market_query.into())?;
    Ok(marketmap_market_response)
}

pub fn query_oracle_currency_pairs(deps: &Deps<NeutronQuery>) -> ContractResult<Vec<CurrencyPair>> {
    let oracle_currency_pairs_query: OracleQuery = OracleQuery::GetAllCurrencyPairs {};
    let oracle_currency_pairs_response: GetAllCurrencyPairsResponse =
        deps.querier.query(&oracle_currency_pairs_query.into())?;
    Ok(oracle_currency_pairs_response.currency_pairs)
}

pub fn query_marketmap_market_map(deps: &Deps<NeutronQuery>) -> ContractResult<MarketMap> {
    let marketmap_currency_pairs_query: MarketMapQuery = MarketMapQuery::MarketMap {};
    let marketmap_currency_pairs_response: MarketMapResponse =
        deps.querier.query(&marketmap_currency_pairs_query.into())?;
    Ok(marketmap_currency_pairs_response.market_map)
}

pub fn validate_market(
    deps: &Deps<NeutronQuery>,
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
    validate_market_enabled(deps, &pair, None)?;
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
    deps: &Deps<NeutronQuery>,
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

    match oracle_price_response.price.block_height {
        Some(block_height) => {
            if (current_block_height - block_height) > max_blocks_old {
                return Err(ContractError::PriceTooOld {
                    symbol: pair.base.clone(),
                    quote: pair.quote.clone(),
                    max_blocks: max_blocks_old,
                });
            }
        }
        None => {
            return Err(ContractError::PriceAgeUnavailable {
                symbol: pair.base.clone(),
                quote: pair.quote.clone(),
            });
        }
    }

    Ok(Response::new())
}

pub fn validate_market_enabled(
    deps: &Deps<NeutronQuery>,
    pair: &CurrencyPair,
    marketmap_market_response: Option<MarketResponse>,
) -> ContractResult<Response> {
    let marketmap_market_response = match marketmap_market_response {
        Some(response) => response,
        None => query_marketmap_market(deps, &pair)?,
    };

    if !marketmap_market_response.market.ticker.enabled {
        return Err(ContractError::UnsupportedMarket {
            symbol: pair.base.clone(),
            quote: pair.quote.clone(),
            location: "x/marketmap".to_string(),
        });
    }

    Ok(Response::new())
}

pub fn validate_market_supported_xoracle(
    deps: &Deps<NeutronQuery>,
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
    deps: &Deps<NeutronQuery>,
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
    deps: &Deps<NeutronQuery>,
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

pub fn get_prices(deps: Deps<NeutronQuery>, env: Env) -> ContractResult<CombinedPriceResponse> {
    let config = load_config(deps)?;

    // Helper function to get price or return 1 if the base is a USD denom
    fn get_price_or_default(
        deps: &Deps<NeutronQuery>,
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

        // Normalize the price
        normalize_price(price_response.price.price, price_response.decimals)
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
        .map_err(|e| ContractError::DecimalConversionError)
}

fn price_ratio(price_1: Decimal, price_2: Decimal) -> Decimal {
    price_1 / price_2
}

pub fn is_usd_denom(currency: &str) -> bool {
    match currency {
        "USD" | "USDC" | "USDT" => true,
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
    deps: &DepsMut<NeutronQuery>,
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
    deps: &DepsMut<NeutronQuery>,
    env: Env,
    config: &mut Config,
) -> Result<(), ContractError> {
    // Query the contract balances for the two tokens
    let balances = query_contract_balance(&deps, env, config.pair_data.clone())?;

    // Update the config balances based on the queried balances
    for coin in balances.iter() {
        if coin.denom == config.pair_data.token_0.denom {
            config.balances.token_0.amount = coin.amount;
        } else if coin.denom == config.pair_data.token_1.denom {
            config.balances.token_1.amount = coin.amount;
        }
    }

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
    deps: &DepsMut<NeutronQuery>,
    env: &Env,
    config: &mut Config,
    prices: &CombinedPriceResponse,
    index: i64,
) -> Result<(Vec<CosmosMsg<NeutronMsg>>, Uint128, Uint128), ContractError> {
    let mut messages: Vec<CosmosMsg<NeutronMsg>> = vec![];
    let target_tick_index_0 = index + config.base_fee as i64;
    let target_tick_index_1 = (index * -1) + config.base_fee as i64;
    let dex_querier = dex::DexQuerier::new(&deps.querier);


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

    let token_1_liquidity_response = dex_querier.tick_liquidity_all(
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

    // get tick index of cheapest token0 liquidity
    if !token_1_liquidity_response.tick_liquidity.is_empty() {
        let liq: &TickLiquidity = &token_1_liquidity_response.tick_liquidity[0];
        let lowest_tick_index_1: i64;
        // Handle empty case
        match &liq.liquidity {
            Some(tick_liquidity::Liquidity::PoolReserves(reserves)) => {
                lowest_tick_index_1 = reserves.key.as_ref().map_or_else(
                    || Err(ContractError::TickIndexDoesNotExist),
                    |key| Ok(key.tick_index_taker_to_maker),
                )?;
            }
            Some(tick_liquidity::Liquidity::LimitOrderTranche(tranche)) => {
                lowest_tick_index_1 = tranche.key.as_ref().map_or_else(
                    || Err(ContractError::TickIndexDoesNotExist),
                    |key| Ok(key.tick_index_taker_to_maker),
                )?;
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
            Some(tick_liquidity::Liquidity::PoolReserves(reserves)) => {
                lowest_tick_index_0 = reserves.key.as_ref().map_or_else(
                    || Err(ContractError::TickIndexDoesNotExist),
                    |key| Ok(key.tick_index_taker_to_maker),
                )?;
            }
            Some(tick_liquidity::Liquidity::LimitOrderTranche(tranche)) => {
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
            token_0_swappable = true;
        }
    }


    let mut swapped_amount_0: Uint128 = Uint128::zero();
    let mut gained_amount_0: Uint128 = Uint128::zero();
    let mut swapped_amount_1: Uint128 = Uint128::zero();
    let mut gained_amount_1: Uint128 = Uint128::zero();

    if (token_0_swappable) {
        // TODO: ensure autoswap is enabled and responce includes the total pricing.
        let token_0_swap_simulation_result = dex_querier.estimate_place_limit_order(
            env.contract.address.to_string(),
            env.contract.address.to_string(),
            config.pair_data.token_0.denom.clone(),
            config.pair_data.token_1.denom.clone(),
            target_tick_index_0 + 2,
            config.balances.token_0.amount.to_string(),
            // TODO: get enum notation
            2,
            None,
            "".to_string(),
        )?;

        // get the amount of token 0 the limit order immediately used
        swapped_amount_0 = token_0_swap_simulation_result
            .swap_in_coin
            .and_then(|coin| Uint128::from_str(&coin.amount).ok())
            .unwrap_or(Uint128::zero());
        // get the amount of token1 that the limit order immediatly produced
        // Q: Does this correctly account for auto-swap pricing? i.e autoswapping through multiple orders
        gained_amount_1 = token_0_swap_simulation_result
            .swap_out_coin
            .and_then(|coin| Uint128::from_str(&coin.amount).ok())
            .unwrap_or(Uint128::zero());
    }

    if (token_1_swappable) {
        // estimate placing a limit order
        let token_1_swap_simulation_result = dex_querier.estimate_place_limit_order(
            env.contract.address.to_string(),
            env.contract.address.to_string(),
            config.pair_data.token_1.denom.clone(),
            config.pair_data.token_0.denom.clone(),
            target_tick_index_1 + 2,
            config.balances.token_1.amount.to_string(),
            2,
            None,
            // TODO: make this an option
            "".to_string(),
        )?;

        // get the amount of token 1 the limit order immediately used
        swapped_amount_1 = token_1_swap_simulation_result
            .swap_in_coin
            .and_then(|coin| Uint128::from_str(&coin.amount).ok())
            .unwrap_or(Uint128::zero());
        // get the amount of token 0 that the limit order immediatly produced
        gained_amount_0 = token_1_swap_simulation_result
            .swap_out_coin
            .and_then(|coin| Uint128::from_str(&coin.amount).ok())
            .unwrap_or(Uint128::zero());
    }


    // let limit_sell_price_0: Decimal = prices.token_0_price * Decimal::percent(120);
    // let limit_sell_price_1: Decimal = prices.token_1_price * Decimal::percent(120);

    if swapped_amount_0 > Uint128::zero() {
        let limit_order_msg = DexMsg::PlaceLimitOrder {
            receiver: env.contract.address.to_string(),
            token_in: config.pair_data.token_0.denom.clone(),
            token_out: config.pair_data.token_1.denom.clone(),
            tick_index_in_to_out: target_tick_index_0 + 2,
            amount_in: swapped_amount_0,
            order_type: LimitOrderType::ImmediateOrCancel,
            expiration_time: None,
            max_amount_out: None,
            // TODO: make this an option
            limit_sell_price: "".to_string(),
        };

        messages.push(CosmosMsg::Custom(NeutronMsg::Dex(limit_order_msg)));

        config.balances.token_0.amount = config
        .balances
        .token_0
        .amount
        .checked_sub(swapped_amount_0)
        .unwrap_or_default();
    }
    if swapped_amount_1 > Uint128::zero() {
        let limit_order_msg = DexMsg::PlaceLimitOrder {
            receiver: env.contract.address.to_string(),
            token_in: config.pair_data.token_1.denom.clone(),
            token_out: config.pair_data.token_0.denom.clone(),
            tick_index_in_to_out: target_tick_index_1 + 2,
            amount_in: swapped_amount_1,
            order_type: LimitOrderType::ImmediateOrCancel,
            expiration_time: None,
            max_amount_out: None,
            // TODO: make this an option
            limit_sell_price: "".to_string(),
        };

        messages.push(CosmosMsg::Custom(NeutronMsg::Dex(limit_order_msg)));

        config.balances.token_1.amount = config
        .balances
        .token_1
        .amount
        .checked_sub(swapped_amount_1)
        .unwrap_or_default();
    }
    Ok((messages, gained_amount_0, gained_amount_1))
}
