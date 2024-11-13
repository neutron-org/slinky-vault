use crate::error::{ContractError, ContractResult};
use crate::state::CONFIG;
use crate::utils::*;
use cosmwasm_std::{
    BankMsg, Coin, CosmosMsg, DepsMut, Env, MessageInfo, Response, SubMsg, SubMsgResult, Uint128,
};
use neutron_std::types::neutron::dex::{
    DepositOptions, DexQuerier, MsgDeposit, MsgWithdrawal, QueryAllUserDepositsResponse,
};

pub fn deposit(deps: DepsMut, _env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Load the contract configuration from storage
    let mut config = CONFIG.load(deps.storage)?;

    // Extract the sent funds from the transaction info
    let sent_funds = info.funds;

    // If no funds are sent, return an error
    if sent_funds.is_empty() {
        return Err(ContractError::NoFundsSent {});
    }

    // Iterate through the sent funds and update the contract's balances
    for coin in sent_funds.iter() {
        if coin.denom == config.balances.token_0.denom {
            config.balances.token_0.amount += coin.amount;
        } else if coin.denom == config.balances.token_1.denom {
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
        .add_attribute("action", "deposit")
        .add_attribute("from", info.sender.to_string())
        .add_attribute("token_0_amount", config.balances.token_0.amount.to_string())
        .add_attribute("token_1_amount", config.balances.token_1.amount.to_string()))
}

pub fn withdraw(deps: DepsMut, _env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Load the contract configuration to access the owner address and balances
    let mut config = CONFIG.load(deps.storage)?;

    // Verify that the sender is the owner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Check if there are any funds to withdraw
    if config.balances.token_0.amount.is_zero() && config.balances.token_1.amount.is_zero() {
        return Err(ContractError::NoFundsAvailable {});
    }

    // Prepare messages to send the entire balance of each token back to the owner
    let mut messages: Vec<CosmosMsg> = vec![];

    if !config.balances.token_0.amount.is_zero() {
        messages.push(
            BankMsg::Send {
                to_address: config.owner.to_string(),
                amount: vec![Coin {
                    denom: config.balances.token_0.denom.clone(),
                    amount: config.balances.token_0.amount,
                }],
            }
            .into(),
        );
    }

    if !config.balances.token_1.amount.is_zero() {
        messages.push(
            BankMsg::Send {
                to_address: config.owner.to_string(),
                amount: vec![Coin {
                    denom: config.balances.token_1.denom.clone(),
                    amount: config.balances.token_1.amount,
                }],
            }
            .into(),
        );
    }

    // Reset the balances to zero after withdrawal
    config.balances.token_0.amount = Uint128::zero();
    config.balances.token_1.amount = Uint128::zero();

    // Save the updated config (with zeroed balances) back to storage
    CONFIG.save(deps.storage, &config)?;

    // Return a successful response with the messages to transfer the funds
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "withdraw_all")
        .add_attribute("owner", config.owner.to_string())
        .add_attribute(
            "token_0_withdrawn",
            config.balances.token_0.amount.to_string(),
        )
        .add_attribute(
            "token_1_withdrawn",
            config.balances.token_1.amount.to_string(),
        ))
}

pub fn dex_deposit(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    // Load the contract configuration
    let mut config = CONFIG.load(deps.storage)?;

    // Verify that the sender is the owner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut messages: Vec<CosmosMsg> = vec![];

    // get the current slinky price and tick index
    let prices: crate::msg::CombinedPriceResponse = get_prices(deps.as_ref(), env.clone())?;
    let tick_index = price_to_tick_index(prices.price_0_to_1)?;

    let (lo_messages, expected_amount0, expected_amount1) =
        prepare_state(&deps, &env, &mut config, &prices, tick_index)?;

    // Save the updated config directly to storage
    CONFIG.save(deps.storage, &config)?;
    // Add lo_messages to messages if not empty

    if !lo_messages.is_empty() {
        messages.extend(lo_messages);
    }

    let deposit_data = get_deposit_data(
        config.balances.token_0.amount,
        config.balances.token_1.amount,
        expected_amount0,
        expected_amount1,
        tick_index,
        config.base_fee,
        &prices,
        config.base_deposit_percentage,
    )?;

    // Update config balances by subtracting deposit amounts
    config.balances.token_0.amount = config
        .balances
        .token_0
        .amount
        .checked_sub(deposit_data.amount0)
        .map_err(|_| ContractError::InsufficientFunds {
            available: config.balances.token_0.amount,
            required: deposit_data.amount0,
        })?;
    config.balances.token_1.amount = config
        .balances
        .token_1
        .amount
        .checked_sub(deposit_data.amount1)
        .map_err(|_| ContractError::InsufficientFunds {
            available: config.balances.token_1.amount,
            required: deposit_data.amount1,
        })?;

    // Save the updated config to ensure the change is persistent
    CONFIG.save(deps.storage, &config)?;

    // Prepare the deposit message with updated balances
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

    // Create the response with the deposit message
    messages.push(dex_msg);
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "dex_deposit"))
}

pub fn dex_withdrawal(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // Load the contract configuration to access the owner address and balances
    let config = CONFIG.load(deps.storage)?;

    // Verify that the sender is the owßner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Prepare a vector to hold withdrawals
    let mut messages: Vec<CosmosMsg> = vec![];

    // Check if there are any active deposits

    let dex_querier = DexQuerier::new(&deps.querier);
    let res: QueryAllUserDepositsResponse =
        dex_querier.user_deposits_all(_env.contract.address.to_string(), None, true)?;
    // let res: AllUserDepositsResponse = deps.querier.query(&query_msg.into())?;

    // If there are any active deposits, withdraw all of them
    for deposit in res.deposits.iter() {
        let withdraw_msg = Into::<CosmosMsg>::into(MsgWithdrawal {
            creator: _env.contract.address.to_string(),
            receiver: _env.contract.address.to_string(),
            token_a: config.pair_data.token_0.denom.clone(),
            token_b: config.pair_data.token_1.denom.clone(),
            shares_to_remove: vec![deposit
                .shares_owned
                .parse()
                .expect("Failed to parse the string as an integer")],
            tick_indexes_a_to_b: vec![deposit.center_tick_index],
            fees: vec![deposit.fee], // Handle `None` case with `unwrap_or`
        });
        // Wrap the DexMsg into a CosmosMsg::Custom
        messages.push(withdraw_msg);
    }

    // Add the message to the response and return
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "dex_deposit"))
}

pub fn handle_reply(
    deps: DepsMut,
    env: Env,
    msg_result: SubMsgResult,
    schedule_id: u64,
) -> Result<Response, ContractError> {
    match msg_result {
        SubMsgResult::Ok(result) => {
            let mut config = CONFIG.load(deps.storage)?;
            // Update balances in the config
            update_contract_balance(&deps, env.clone(), &mut config)?;

            // Save the updated config directly to storage
            CONFIG.save(deps.storage, &config)?;
            Ok(Response::new()
                .add_attribute("action", "place_limit_order_reply_success")
                .add_attribute("schedule_id", schedule_id.to_string()))
        }
        SubMsgResult::Err(err) => Ok(Response::new()
            .add_attribute("action", "place_limit_order_reply_error")
            .add_attribute("error", err)
            .add_attribute("schedule_id", schedule_id.to_string())),
    }
}
