use crate::error::ContractError;
use crate::state::{CONFIG, DEX_WITHDRAW_REPLY_ID};
use crate::utils::*;
use cosmwasm_std::{CosmosMsg, DepsMut, Env, MessageInfo, Response, SubMsg, SubMsgResult, Uint128};
use neutron_std::types::neutron::dex::{DexQuerier, MsgWithdrawal, QueryAllUserDepositsResponse};

pub fn deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let messages: Vec<CosmosMsg> = vec![];
    // Load the contract configuration from storage
    let mut config = CONFIG.load(deps.storage)?;
    //if calles is not the owner error
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }
    // Extract the sent funds from the transaction info
    let sent_funds = info.funds;

    // If no funds are sent, return an error
    if sent_funds.is_empty() {
        return Err(ContractError::NoFundsSent {});
    }
    let mut token0_deposited = Uint128::zero();
    let mut token1_deposited = Uint128::zero();
    // Iterate through the sent funds if the denoms match the expected vault denom, and are greater than zero we add them to the config balances.
    for coin in sent_funds.iter() {
        if coin.denom == config.balances.token_0.denom {
            if coin.amount == Uint128::zero() {
                return Err(ContractError::InvalidTokenAmount);
            }
            token0_deposited += coin.amount;
            config.balances.token_0.amount += coin.amount;
        } else if coin.denom == config.balances.token_1.denom {
            if coin.amount == Uint128::zero() {
                return Err(ContractError::InvalidTokenAmount);
            }
            token1_deposited += coin.amount;
            config.balances.token_1.amount += coin.amount;
        } else {
            // Return an error if an unsupported token is sent
            return Err(ContractError::InvalidToken);
        }
    }

    // Save the updated configuration with new balances back to the contract's storage
    CONFIG.save(deps.storage, &config)?;

    // Return a success response with updated balances
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "deposit")
        .add_attribute("from", info.sender.to_string())
        .add_attribute("token_0_amount", config.balances.token_0.amount.to_string())
        .add_attribute("token_1_amount", config.balances.token_1.amount.to_string()))
}

pub fn withdraw(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Load the contract configuration to access the owner address and balances
    let config = CONFIG.load(deps.storage)?;
    let mut messages: Vec<SubMsg> = vec![];

    // Verify that the sender is the owner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let dex_querier = DexQuerier::new(&deps.querier);
    let res: QueryAllUserDepositsResponse =
        dex_querier.user_deposits_all(env.contract.address.to_string(), None, true)?;

    // Add all withdrawals except the last one without reply
    for deposit in res.deposits.iter() {
        let withdraw_msg = Into::<CosmosMsg>::into(MsgWithdrawal {
            creator: env.contract.address.to_string(),
            receiver: env.contract.address.to_string(),
            token_a: config.pair_data.token_0.denom.clone(),
            token_b: config.pair_data.token_1.denom.clone(),
            shares_to_remove: vec![deposit.shares_owned.parse().expect("Failed to parse")],
            tick_indexes_a_to_b: vec![deposit.center_tick_index],
            fees: vec![deposit.fee],
        });
        messages.push(SubMsg::new(withdraw_msg));
    }

    // Add the message to the response and return
    Ok(Response::new()
        .add_submessages(messages)
        .add_attribute("action", "withdrawal"))
}

// depends on up-to-date config
pub fn dex_deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Load the contract configuration
    let config = CONFIG.load(deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];

    // Verify that the sender is the owner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // get the current slinky price and tick index
    let prices: crate::msg::CombinedPriceResponse = get_prices(deps.as_ref(), env.clone())?;
    let tick_index = price_to_tick_index(prices.price_0_to_1)?;

    let (lo_messages, token_0_usable, token_1_usable) =
        prepare_state(&deps, &env, &config, tick_index)?;
    messages.extend(lo_messages);
    let deposit_messages = get_deposit_messages(
        &env,
        config.clone(),
        tick_index,
        prices,
        token_0_usable,
        token_1_usable,
    )?;
    messages.extend(deposit_messages);

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "dex_deposit"))
}

pub fn dex_withdrawal(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // Load the contract configuration to access the owner address and balances
    let config = CONFIG.load(deps.storage)?;

    // Verify that the sender is the owner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Prepare a vector to hold withdrawals
    let mut messages: Vec<SubMsg> = vec![];
    // Check if there are any active deposits
    let dex_querier = DexQuerier::new(&deps.querier);
    let res: QueryAllUserDepositsResponse =
        dex_querier.user_deposits_all(env.contract.address.to_string(), None, true)?;

    // If there are any active deposits, withdraw all of them
    for deposit in res.deposits.iter() {
        let withdraw_msg = Into::<CosmosMsg>::into(MsgWithdrawal {
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
        });

        // Wrap the DexMsg into a SubMsg with reply
        messages.push(SubMsg::reply_always(withdraw_msg, DEX_WITHDRAW_REPLY_ID));
    }

    // Add the message to the response and return
    Ok(Response::new()
        .add_submessages(messages)
        .add_attribute("action", "dex_withdrawal"))
}

pub fn handle_dex_withdrawal_reply(
    deps: DepsMut,
    env: Env,
    msg_result: SubMsgResult,
) -> Result<Response, ContractError> {
    match msg_result {
        SubMsgResult::Ok(result) => {
            let mut config = CONFIG.load(deps.storage)?;
            let (amount0, amount1) = extract_withdrawal_amounts(&result)?;

            config.balances.token_0.amount += amount0;
            config.balances.token_1.amount += amount1;

            CONFIG.save(deps.storage, &config)?;

            Ok(Response::new().add_attribute("action", "withdrawal_reply_success"))
        }
        SubMsgResult::Err(err) => Ok(Response::new()
            .add_attribute("action", "withdrawal_reply_error")
            .add_attribute("error", err)),
    }
}
