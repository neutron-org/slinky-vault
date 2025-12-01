use std::str::FromStr;

use crate::error::{ContractError, ContractResult};
use crate::external_types::{CoreQueryMsg, DualRedemptionRateResponse};
use crate::state::{CombinedPriceResponse, TokenData, CONFIG};
use cosmwasm_std::{Decimal, Deps, Env, Int128, Response};
use neutron_std::types::neutron::util::precdec::PrecDec;
use neutron_std::types::slinky::{
    marketmap::v1::{MarketMap, MarketResponse, MarketmapQuerier},
    oracle::v1::{GetAllCurrencyPairsResponse, GetPriceResponse, OracleQuerier},
    types::v1::CurrencyPair,
};

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
    validate_market_supported_xoracle(deps, pair, None)?;
    validate_market_supported_xmarketmap(deps, pair, None)?;
    validate_price_recent(
        deps,
        env,
        pair,
        max_blocks_old,
        Some(price_response.clone()),
    )?;
    validate_price_not_nil(deps, pair, Some(price_response.clone()))?;
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
        None => query_oracle_price(deps, pair)?,
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
        None => query_marketmap_market(deps, pair)?,
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
    if !map.markets.contains_key(&key) {
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
        None => query_oracle_price(deps, pair)?,
    };

    if oracle_price_response.nonce == 0 {
        return Err(ContractError::PriceIsNil {
            symbol: pair.base.clone(),
            quote: pair.quote.clone(),
        });
    }
    Ok(Response::new())
}

pub fn get_prices(
    deps: Deps,
    env: Env,
    token_a: TokenData,
    token_b: TokenData,
) -> ContractResult<CombinedPriceResponse> {
    let config = CONFIG.load(deps.storage)?;
    
    // Create currency pairs for Slinky price query
    let pair_1 = CurrencyPair {
        base: token_a.pair.base.to_string(),
        quote: token_a.pair.quote.to_string(),
    };
    let pair_2 = CurrencyPair {
        base: token_b.pair.base.to_string(),
        quote: token_b.pair.quote.to_string(),
    };

    // Helper function to get the slinky price (return 1 if the base is a USD denom)
    fn get_price_or_default(
        deps: &Deps,
        env: &Env,
        pair: &CurrencyPair,
        max_blocks_old: u64,
    ) -> ContractResult<PrecDec> {
        // Check if the pair's base is USD denom
        if is_usd_denom(&pair.base) {
            return Ok(PrecDec::one());
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

    // Get base prices for both tokens, normalized by decimals
    let mut token_0_price =
        get_price_or_default(&deps, &env, &pair_1, token_a.max_blocks_old)?.checked_div(
            PrecDec::from_ratio(10u128.pow(token_a.decimals.into()), 1u128),
        )?;

    let mut token_1_price =
        get_price_or_default(&deps, &env, &pair_2, token_b.max_blocks_old)?.checked_div(
            PrecDec::from_ratio(10u128.pow(token_b.decimals.into()), 1u128),
        )?;

    // Apply redemption rates based on token type
    // Check if token_a is maxBTC or LST
    if token_a.denom.eq(&config.maxbtc_denom) {
        // token_a is maxBTC, get its redemption rate from the core contract
        let maxbtc_exchange_rate = query_maxbtc_exchange_rate(deps, env.clone())?;
        let maxbtc_redemption_rate = PrecDec::from_str(&maxbtc_exchange_rate.to_string())
            .map_err(|_| ContractError::PrecDecConversionError)?;
        token_0_price = token_0_price.checked_mul(maxbtc_redemption_rate)?;
        
        //   mint_fee = 25: (10000 - 25) / 10000 = 9975 / 10000 = 0.9975 (discount)
        let adjustment_factor = PrecDec::from_ratio(
            (10000i128 - config.maxbtc_mint_fee as i128) as u128, 
            10000u128
        );
        token_0_price = token_0_price.checked_mul(adjustment_factor)?;
    } else if token_a.denom.eq(&config.lst_denom) {
        // token_a is LST, use the stored redemption rate
        token_0_price = token_0_price.checked_mul(config.lst_redemption_rate)?;
    }

    // Check if token_b is maxBTC or LST
    if token_b.denom.eq(&config.maxbtc_denom) {
        // token_b is maxBTC, get its redemption rate from the core contract
        let maxbtc_exchange_rate = query_maxbtc_exchange_rate(deps, env.clone())?;
        let maxbtc_redemption_rate = PrecDec::from_str(&maxbtc_exchange_rate.to_string())
            .map_err(|_| ContractError::PrecDecConversionError)?;
        token_1_price = token_1_price.checked_mul(maxbtc_redemption_rate)?;

        let adjustment_factor = PrecDec::from_ratio(
            (10000i128 - config.maxbtc_mint_fee as i128) as u128, 
            10000u128
        );
        token_1_price = token_1_price.checked_mul(adjustment_factor)?;
    } else if token_b.denom.eq(&config.lst_denom) {
        // token_b is LST, use the stored redemption rate
        token_1_price = token_1_price.checked_mul(config.lst_redemption_rate)?;
    }

    // Calculate the price ratio
    let price_0_to_1 = price_ratio(token_0_price, token_1_price);
    
    // Return the prices and their ratio
    let res = CombinedPriceResponse {
        token_0_price,
        token_1_price,
        price_0_to_1,
    };

    Ok(res)
}

// Helper function to normalize the price to PrecDec
pub fn normalize_price(price: Int128, decimals: u64) -> ContractResult<PrecDec> {
    // Ensure decimals does not exceed u32::MAX
    if decimals > u32::MAX as u64 {
        return Err(ContractError::TooManyDecimals);
    }
    if price < Int128::zero() {
        return Err(ContractError::PriceIsNegative);
    }
    let abs_value: u128 = price.i128().unsigned_abs();
    PrecDec::from_atomics(abs_value, decimals as u32)
        .map_err(|_e| ContractError::DecimalConversionError)
}

fn price_ratio(price_1: PrecDec, price_2: PrecDec) -> PrecDec {
    price_1 / price_2
}

pub fn query_maxbtc_exchange_rate(deps: Deps, _env: Env) -> ContractResult<Decimal> {
    let config = CONFIG.load(deps.storage)?;
    let exchange_rate: Decimal = deps
        .querier
        .query_wasm_smart(config.maxbtc_core_contract.clone(), &CoreQueryMsg::ExchangeRate {})?;

    Ok(exchange_rate)
}

pub fn query_dual_redemption_rates(deps: Deps, env: Env) -> ContractResult<DualRedemptionRateResponse> {
    let config = CONFIG.load(deps.storage)?;
    
    // Get maxBTC redemption rate from the core contract
    let maxbtc_exchange_rate = query_maxbtc_exchange_rate(deps, env.clone())?;
    
    // Get LST redemption rate from config
    let lst_redemption_rate = PrecDec::to_string(&config.lst_redemption_rate);
    let lst_redemption_rate_decimal = Decimal::from_str(&lst_redemption_rate)
        .map_err(|_| ContractError::DecimalConversionError)?;
    
    Ok(DualRedemptionRateResponse {
        maxbtc_redemption_rate: maxbtc_exchange_rate,
        lst_redemption_rate: lst_redemption_rate_decimal,
        maxbtc_update_time: env.block.time.seconds(),
        lst_update_time: env.block.time.seconds(), // We could store separate update time for LST if needed
    })
}

pub fn is_usd_denom(currency: &str) -> bool {
    matches!(currency, "USD" | "USDC")
}
