use crate::error::ContractError;
use crate::execute::*;
use crate::msg::{CombinedPriceResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::query::*;
use crate::state::{Balances, Config, PairData, TokenData, CONFIG};
use crate::utils::*;

use cosmwasm_std::{
    attr, entry_point, to_json_binary, Binary, Coin, Deps, DepsMut, Env, Int128, MessageInfo,
    Response, StdResult, Uint128, Uint64,
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

///////////////
/// MIGRATE ///
///////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> ContractResult<Response> {
    unimplemented!()
}

const CONTRACT_NAME: &str = concat!("crates.io:neutron-contracts__", env!("CARGO_PKG_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

///////////////////
/// INSTANTIATE ///
///////////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut<NeutronQuery>,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    msg.validate()?;
    let owner = deps.api.addr_validate(&msg.owner)?;
    let token_a = msg.token_a.clone();
    let token_b = msg.token_b.clone();
    let (tokens, id) = sort_token_data_and_get_pair_id_str(&token_a, &token_b);
    let deps_readonly = Deps {
        storage: deps.storage,
        api: deps.api,
        querier: deps.querier,
    };
    validate_market(&deps_readonly, &_env, &msg.token_a.pair, msg.max_block_old)?;
    validate_market(&deps_readonly, &_env, &msg.token_b.pair, msg.max_block_old)?;

    let pairs = PairData {
        token_0: tokens[0].clone(),
        token_1: tokens[1].clone(),
        pair_id: id.clone(),
    };

    let balances = Balances {
        token_0: Coin::new(Uint128::zero(), tokens[0].denom.clone()),
        token_1: Coin::new(Uint128::zero(), tokens[1].denom.clone()),
    };

    let config = Config {
        pair_data: pairs.clone(),
        max_blocks_old: msg.max_block_old,
        balances,
        base_fee: msg.base_fee,
        base_deposit_percentage: msg.base_deposit_percentage,
        owner,
    };

    // PAIRDATA.save(deps.storage, &pool_data)?;
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attributes([
            attr("owner", config.owner.to_string()),
            attr("max_blocks_stale", config.max_blocks_old.to_string()),
            attr("token_0_denom", pairs.token_0.denom),
            attr("token_0_symbol", pairs.token_0.pair.base),
            attr("token_0_quote_currency", pairs.token_0.pair.quote),
            attr("token_1_denom", pairs.token_1.denom),
            attr("token_1_symbol", pairs.token_1.pair.base),
            attr("token_1_quote_currency", pairs.token_1.pair.quote),
            attr("pool_id", pairs.pair_id),
        ]))
}

///////////////
/// EXECUTE ///
///////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut<NeutronQuery>,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response<NeutronMsg>, ContractError> {
    match msg {
        ExecuteMsg::Deposit { .. } => deposit(deps, _env, info),
        ExecuteMsg::Withdraw { .. } => {
            // Prevent tokens from being sent with the Withdraw message
            if !info.funds.is_empty() {
                return Err(ContractError::FundsNotAllowed);
            }
            withdraw(deps, _env, info)
        }
        ExecuteMsg::DexDeposit { .. } => {
            // Prevent tokens from being sent with the Withdraw message
            if !info.funds.is_empty() {
                return Err(ContractError::FundsNotAllowed);
            }
            dex_deposit(deps, _env, info)
        }
        ExecuteMsg::DexWithdrawal { .. } => {
            // Prevent tokens from being sent with the Withdraw message
            if !info.funds.is_empty() {
                return Err(ContractError::FundsNotAllowed);
            }
            dex_withdrawal(deps, _env, info)
        }
    }
}

/////////////
/// QUERY ///
/////////////

#[entry_point]
pub fn query(deps: Deps<NeutronQuery>, _env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::GetPrice {
            base_symbol,
            quote_currency,
            max_blocks_old,
        } => query_recent_valid_price(deps, _env, base_symbol, quote_currency, max_blocks_old), // handle other queries
        QueryMsg::GetFormated {} => query_recent_valid_prices_formatted(deps, _env),
        QueryMsg::GetDeposits {} => q_dex_deposit(deps, _env),
    }
}

pub fn QuerySlinkyPrice(deps: Deps, _env: Env) -> ContractResult<Binary> {
    Err(ContractError::BadTokenA)
}
