use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{Balances, Config, PairData, TokenData, CONFIG};
use crate::utils::*;

use cosmwasm_std::{
    attr, entry_point, to_json_binary, BankMsg, Binary, Coin, CosmosMsg, Deps, DepsMut, Env,
    Int128, MessageInfo, QueryRequest, Response, StdResult, Uint128, Uint64,
};
use cw2::set_contract_version;

pub type ContractResult<T> = core::result::Result<T, ContractError>;
use neutron_sdk::bindings::marketmap::query::{MarketMapQuery, MarketMapResponse, MarketResponse};
use neutron_sdk::bindings::marketmap::types::MarketMap;
use neutron_sdk::bindings::oracle::query::{
    GetAllCurrencyPairsResponse, GetPriceResponse, GetPricesResponse, OracleQuery,
};

use neutron_sdk::bindings::dex::msg::DexMsg;
use neutron_sdk::bindings::dex::query::{AllUserDepositsResponse, DexQuery};
use neutron_sdk::bindings::dex::types::DepositOption;
use neutron_sdk::proto_types::neutron::dex;
use neutron_sdk::proto_types::neutron::dex::QueryAllUserDepositsResponse;

use neutron_sdk::bindings::oracle::types::CurrencyPair;
use neutron_sdk::bindings::{msg::NeutronMsg, query::NeutronQuery};
use serde_json::to_string;

pub fn deposit(
    deps: DepsMut<NeutronQuery>,
    _env: Env,
    info: MessageInfo,
) -> Result<Response<NeutronMsg>, ContractError> {
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

pub fn withdraw(
    deps: DepsMut<NeutronQuery>,
    _env: Env,
    info: MessageInfo,
) -> Result<Response<NeutronMsg>, ContractError> {
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
    let mut messages: Vec<cosmwasm_std::CosmosMsg<NeutronMsg>> = vec![];

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
    Ok(Response::<NeutronMsg>::new()
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

pub fn dex_deposit(
    deps: DepsMut<NeutronQuery>,
    env: Env,
    info: MessageInfo,
) -> Result<Response<NeutronMsg>, ContractError> {
    // Load the contract configuration
    let mut config = CONFIG.load(deps.storage)?;

    // Verify that the sender is the owner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut messages: Vec<CosmosMsg<NeutronMsg>> = vec![];

    // get the current slinky price and tick index
    let prices = get_prices(deps.as_ref(), env.clone())?;
    let tick_index = price_to_tick_index(prices.price_0_to_1)?;

    // Update balances in the config
    update_contract_balance(&deps, env.clone(), &mut config)?;

    // Save the updated config directly to storage
    CONFIG.save(deps.storage, &config)?;

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
    let dex_msg = DexMsg::Deposit {
        receiver: env.contract.address.to_string(),
        token_a: config.pair_data.token_0.denom.clone(),
        token_b: config.pair_data.token_1.denom.clone(),
        amounts_a: vec![deposit_data.amount0],
        amounts_b: vec![deposit_data.amount1],
        tick_indexes_a_to_b: vec![deposit_data.tick_index],
        fees: vec![deposit_data.fee],
        options: vec![DepositOption {
            disable_swap: false,
        }],
    };

    // Create the response with the deposit message
    messages.push(CosmosMsg::Custom(NeutronMsg::Dex(dex_msg)));
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "dex_deposit"))
}

pub fn dex_withdrawal(
    deps: DepsMut<NeutronQuery>,
    _env: Env,
    info: MessageInfo,
) -> Result<Response<NeutronMsg>, ContractError> {
    // Load the contract configuration to access the owner address and balances
    let mut config = CONFIG.load(deps.storage)?;

    // Verify that the sender is the owßner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Prepare a vector to hold withdrawals
    let mut messages: Vec<CosmosMsg<NeutronMsg>> = vec![];

    // Check if there are any active deposits

    let dex_querier = dex::DexQuerier::new(&deps.querier);
    let res: QueryAllUserDepositsResponse =
        dex_querier.user_deposits_all(_env.contract.address.to_string(), None, true)?;
    // let res: AllUserDepositsResponse = deps.querier.query(&query_msg.into())?;

    // If there are any active deposits, withdraw all of them
    for deposit in res.deposits.iter() {
        let withdraw_msg = DexMsg::Withdrawal {
            receiver: _env.contract.address.to_string(),
            token_a: config.pair_data.token_0.denom.clone(),
            token_b: config.pair_data.token_1.denom.clone(),
            shares_to_remove: vec![deposit
                .shares_owned
                .parse()
                .expect("Failed to parse the string as an integer")],
            tick_indexes_a_to_b: vec![deposit.center_tick_index],
            fees: vec![deposit.fee], // Handle `None` case with `unwrap_or`
        };
        // Wrap the DexMsg into a CosmosMsg::Custom
        messages.push(CosmosMsg::Custom(NeutronMsg::Dex(withdraw_msg)));
    }

    // Add the message to the response and return
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "dex_deposit"))
}
