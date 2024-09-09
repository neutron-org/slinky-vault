use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, CombinedPriceResponse};
use crate::state::{Config, PairData, TokenData, CONFIG, Balances};
use crate::utils::*;
use crate::execute::*;
use neutron_sdk::bindings::dex::query::{DexQuery, AllUserDepositsResponse};
use neutron_sdk::proto_types::neutron::dex;

use cosmwasm_std::{entry_point,
    attr, to_json_binary, Binary, Deps, DepsMut, Env, Int128, MessageInfo, Response, StdResult,
    Uint64, Coin, Uint128, Decimal
};
use cw2::set_contract_version;

pub type ContractResult<T> = core::result::Result<T, ContractError>;
use neutron_sdk::bindings::marketmap::query::{MarketMapQuery, MarketMapResponse, MarketResponse};
use neutron_sdk::bindings::marketmap::types::MarketMap;
use neutron_sdk::bindings::oracle::query::{
    GetAllCurrencyPairsResponse, GetPriceResponse, GetPricesResponse, OracleQuery,
};
use neutron_sdk::bindings::oracle::types::CurrencyPair;
use neutron_sdk::bindings::{msg::NeutronMsg, query::NeutronQuery};

pub fn query_recent_valid_prices_formatted(
    deps: Deps<NeutronQuery>,
    env: Env,
) -> ContractResult<Binary> {
    let combined_responce: CombinedPriceResponse = get_prices(deps, env)?;

    return Ok(to_json_binary(&combined_responce)?);
}

pub fn query_recent_valid_price(
    deps: Deps<NeutronQuery>,
    env: Env,
    base_symbol: String,
    quote_currency: String,
    max_blocks_old: Uint64,
) -> ContractResult<Binary> {

    // 1. check if "symbol" in x/oracle and x/marketmap

    // create a CurrencyPair object
    let currency_pair: CurrencyPair = CurrencyPair {
        base: base_symbol.clone(),
        quote: quote_currency.clone(),
    };

    // fetch all supported currency pairs in x/oracle module
    let oracle_currency_pairs_query: OracleQuery = OracleQuery::GetAllCurrencyPairs {};
    let oracle_currency_pairs_response: GetAllCurrencyPairsResponse =
        deps.querier.query(&oracle_currency_pairs_query.into())?;
    if oracle_currency_pairs_response
        .currency_pairs
        .contains(&currency_pair)
        == false
    {
        return Err(ContractError::UnsupportedMarket {
            symbol: currency_pair.base.clone(),
            quote: currency_pair.quote.clone(),
            location: "x/oracle".to_string(),
        });
    }


    let key: String = format!("{}/{}", currency_pair.base, currency_pair.quote);
        
    // fetch all supported currency pairs in x/marketmap module
    let marketmap_currency_pairs_query: MarketMapQuery = MarketMapQuery::MarketMap {};
    let marketmap_currency_pairs_response: MarketMapResponse =
        deps.querier.query(&marketmap_currency_pairs_query.into())?;
    if marketmap_currency_pairs_response
        .market_map
        .markets
        .contains_key(&key)
        == false
    {
        return Err(ContractError::UnsupportedMarket {
            symbol: currency_pair.base.clone(),
            quote: currency_pair.quote.clone(),
            location: "x/marketmap".to_string(),
        });
    }

    // 2. check if "symbol" enabled in x/marketmap

    // fetch market for currency_pair in x/marketmap module
    let marketmap_market_query: MarketMapQuery = MarketMapQuery::Market {
        currency_pair: currency_pair.clone(),
    };
    let marketmap_market_response: MarketResponse =
        deps.querier.query(&marketmap_market_query.into())?;

    // check if currency_pair is enabled
    if marketmap_market_response.market.ticker.enabled == false {
        return Err(ContractError::UnsupportedMarket {
            symbol: currency_pair.base.clone(),
            quote: currency_pair.quote.clone(),
            location: "x/marketmap".to_string(),
        });
    }

    // 3. check if block_timestamp is not too old

    // get current_block_height
    let current_block_height: u64 = env.block.height;

    // fetch price for currency_pair from x/oracle module
    let oracle_price_query: OracleQuery = OracleQuery::GetPrice {
        currency_pair: currency_pair.clone(),
    };
    let oracle_price_response: GetPriceResponse = deps.querier.query(&oracle_price_query.into())?;

    match oracle_price_response.price.block_height {
        Some(block_height) => {
            if (current_block_height - block_height) > max_blocks_old.u64() {
                return Err(ContractError::PriceTooOld {
                    symbol: currency_pair.base.clone(),
                    quote: currency_pair.quote.clone(),
                    max_blocks: max_blocks_old.u64(),
                });
            }
        }
        None => {
            return Err(ContractError::PriceAgeUnavailable {
                symbol: currency_pair.base.clone(),
                quote: currency_pair.quote.clone(),
            });
        }
    }

    // 4. fetch the price from x/oracle module
    let market_price: Int128 = oracle_price_response.price.price;

    // 5. make sure the price value is not None
    if oracle_price_response.nonce == 0 {
        return Err(ContractError::PriceIsNil {
            symbol: currency_pair.base.clone(),
            quote: currency_pair.quote.clone(),
        });
    }
    // 6. return the price as response with proper metadata
    Ok(to_json_binary(&oracle_price_response)?)
}
pub fn q_dex_deposit(deps: Deps<NeutronQuery>, _env: Env) -> ContractResult<Binary> {
    let dex_querier = dex::DexQuerier::new(&deps.querier);
    Ok(to_json_binary(&dex_querier.user_deposits_all(
        _env.contract.address.to_string(),
        None,
        true,
    )?)?)
}

