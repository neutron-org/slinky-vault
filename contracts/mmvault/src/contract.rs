use crate::error::{ContractError, ContractResult};
use crate::execute::*;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, WithdrawPayload};
use crate::query::*;
use crate::state::{
    Config, PairData, CONFIG, CREATE_TOKEN_REPLY_ID, DEX_DEPOSIT_REPLY_ID, WITHDRAW_REPLY_ID,
};
use crate::utils::*;
use cosmwasm_std::{
    attr, entry_point, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, Uint128,
};
use cw2::set_contract_version;
use prost::Message;
use serde_json::to_vec;
use std::str::FromStr;

const CONTRACT_NAME: &str = concat!("crates.io:neutron-contracts__", env!("CARGO_PKG_NAME"));
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

///////////////
/// MIGRATE ///
///////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    // Update contract version
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Load existing config
    let mut config = CONFIG.load(deps.storage)?;

    // Only update config if a new one is provided
    if let Some(new_config) = msg.config {
        config = new_config;
        // Save updated config
        CONFIG.save(deps.storage, &config)?;
    }

    Ok(Response::new()
        .add_attribute("action", "migrate")
        .add_attribute("contract", CONTRACT_NAME)
        .add_attribute("version", CONTRACT_VERSION))
}

///////////////////
/// INSTANTIATE ///
///////////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> ContractResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    msg.validate()?;
    let whitelist = msg
        .whitelist
        .iter()
        .map(|addr| deps.api.addr_validate(addr).map_err(ContractError::Std))
        .collect::<Result<Vec<Addr>, ContractError>>()?;
    let token_a = msg.token_a.clone();
    let token_b = msg.token_b.clone();
    let (tokens, id) = sort_token_data_and_get_pair_id_str(&token_a, &token_b);
    let deps_readonly = Deps {
        storage: deps.storage,
        api: deps.api,
        querier: deps.querier,
    };

    let oracle_contract = deps_readonly.api.addr_validate(&msg.oracle_contract)?;

    let pairs = PairData {
        token_0: tokens[0].clone(),
        token_1: tokens[1].clone(),
        pair_id: id.clone(),
    };

    let fee_tier_config = msg.fee_tier_config;
    let config = Config {
        pair_data: pairs.clone(),
        fee_tier_config,
        lp_denom: "".to_string(),
        total_shares: Uint128::zero(),
        whitelist,
        deposit_cap: msg.deposit_cap,
        last_executed: 0,
        pause_block: 0,
        timestamp_stale: msg.timestamp_stale,
        paused: msg.paused,
        oracle_contract: oracle_contract.clone(),
        skew: false,
        imbalance: 50u32,
        oracle_price_skew: msg.oracle_price_skew,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate IMM")
        .add_attributes([
            attr("owner", format!("{:?}", config.whitelist)),
            attr(
                "max_blocks_stale_token_a",
                config.pair_data.token_0.max_blocks_old.to_string(),
            ),
            attr(
                "max_blocks_stale_token_b",
                config.pair_data.token_1.max_blocks_old.to_string(),
            ),
            attr("token_0_denom", pairs.token_0.denom),
            attr("token_0_symbol", pairs.token_0.pair.base),
            attr("token_0_exponent", pairs.token_0.decimals.to_string()),
            attr("token_0_quote_currency", pairs.token_0.pair.quote),
            attr("token_1_denom", pairs.token_1.denom),
            attr("token_1_symbol", pairs.token_1.pair.base),
            attr("token_1_exponent", pairs.token_1.decimals.to_string()),
            attr("token_1_quote_currency", pairs.token_1.pair.quote),
            attr("pool_id", pairs.pair_id),
            attr("deposit_cap", config.deposit_cap.to_string()),
            attr("oracle_contract", config.oracle_contract.to_string()),
            attr("imbalance", config.imbalance.to_string()),
            attr("fee_tier_config", config.fee_tier_config.to_string()),
            attr("timestamp_stale", config.timestamp_stale.to_string()),
            attr("paused", config.paused.to_string()),
        ]))
}

///////////////
/// EXECUTE ///
///////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Deposit { .. } => deposit(deps, _env, info),
        ExecuteMsg::Withdraw { amount } => {
            // Prevent other tokens from being sent with the Withdraw message
            if info.funds.len() != 1 {
                return Err(ContractError::OnlyLpTokenAllowed);
            }

            let config = CONFIG.load(deps.storage)?;
            let lp_token = info.funds.first().unwrap();

            if lp_token.denom != config.lp_denom || lp_token.amount != amount {
                return Err(ContractError::LpTokenError);
            }
            if amount.is_zero() {
                return Err(ContractError::ZeroBurnAmount);
            }
            withdraw(deps, _env, info, amount)
        }
        ExecuteMsg::DexDeposit { .. } => {
            // Prevent tokens from being sent with the DexDeposit message
            if !info.funds.is_empty() {
                return Err(ContractError::FundsNotAllowed);
            }
            dex_deposit(deps, _env, info)
        }
        ExecuteMsg::DexWithdrawal { .. } => {
            // Prevent tokens from being sent with the DexWithdrawal message
            if !info.funds.is_empty() {
                return Err(ContractError::FundsNotAllowed);
            }
            dex_withdrawal(deps, _env, info)
        }
        ExecuteMsg::CreateToken { .. } => {
            // Prevent tokens from being sent with the CreateToken message
            if !info.funds.is_empty() {
                return Err(ContractError::FundsNotAllowed);
            }
            execute_create_token(deps, _env, info)
        }
        ExecuteMsg::UpdateConfig { update } => {
            // Prevent tokens from being sent with the UpdateConfig message
            if !info.funds.is_empty() {
                return Err(ContractError::FundsNotAllowed);
            }
            update_config(deps, _env, info, update)
        }
    }
}

/////////////
/// QUERY ///
/////////////

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> ContractResult<Binary> {
    match msg {
        QueryMsg::GetDeposits {} => q_dex_deposit(deps, _env),
        QueryMsg::GetConfig {} => query_config(deps, _env),
        QueryMsg::GetPrices {} => {
            let prices = get_prices(deps, _env)?;
            let serialized_prices =
                to_vec(&prices).map_err(|_| ContractError::SerializationError)?;
            Ok(Binary::from(serialized_prices))
        }
        QueryMsg::GetBalance {} => {
            let balance = query_balance(deps, _env)?;
            Ok(balance)
        }
        QueryMsg::SimulateProvideLiquidity {
            amount_0,
            amount_1,
            sender,
        } => {
            let mint_amount = simulate_provide_liquidity(deps, _env, amount_0, amount_1, sender)?;
            Ok(mint_amount)
        }
        QueryMsg::SimulateWithdrawLiquidity { amount } => {
            let burn_amount = simulate_withdraw_liquidity(deps, _env, amount)?;
            Ok(burn_amount)
        }
    }
}

/////////////
/// REPLY ///
/////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        CREATE_TOKEN_REPLY_ID => handle_create_token_reply(deps, msg.result),
        WITHDRAW_REPLY_ID => {
            // Get the payload from the reply data
            let payload = WithdrawPayload::decode(msg.payload.as_slice())
                .map_err(|_| ContractError::ParseError)?;

            let amount =
                Uint128::from_str(&payload.amount).map_err(|_| ContractError::ParseError)?;

            handle_withdrawal_reply(deps, env, msg.result, amount, payload.sender)
        }
        DEX_DEPOSIT_REPLY_ID => {
            if let Err(err) = msg.result.clone().into_result() {
                // Log the error but don't propagate it
                return Ok(Response::new()
                    .add_attribute("action", "dex_deposit_response")
                    .add_attribute("status", "error_handled")
                    .add_attribute("error", format!("{:?}", err)));
            }
            // If successful return success status
            Ok(Response::new()
                .add_attribute("action", "dex_deposit_response")
                .add_attribute("status", "success"))
        }
        id => Err(ContractError::UnknownReplyId { id }),
    }
}
