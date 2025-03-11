use crate::contract::{execute, instantiate, query, reply};
use crate::msg::{CombinedPriceResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{FeeTier, FeeTierConfig, TokenData, CONFIG, CREATE_TOKEN_REPLY_ID};
use crate::testing::mock_querier::{mock_dependencies, WasmMockQuerier};
use cosmwasm_std::{
    from_binary, Binary, Empty, Env, MessageInfo, OwnedDeps, Reply,
    SubMsgResponse, SubMsgResult, Uint128,
};
use cosmwasm_std::testing::{message_info, mock_env, MockApi, MockStorage};
use neutron_std::types::neutron::util::precdec::PrecDec;
use neutron_std::types::osmosis::tokenfactory::v1beta1::MsgCreateDenomResponse;
use neutron_std::types::slinky::types::v1::CurrencyPair;
use prost::Message;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MockMsgResponse {
    pub new_token_denom: String,
}

#[test]
fn test_instantiate_success() {
    // Set up a mock environment with default values
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);

    // Create a sample InstantiateMsg
    let owner_addr = deps.api.addr_make("neutron1e2c5p8y5rw2hp4fjr05uvkrkz76ej0kqegnwxe");
    let caller_addr = deps.api.addr_make("neutron1e2c5p8y5rw2hp4fjr05uvkrkz76ej0kqegnwxe");
    let instantiate_msg = InstantiateMsg {
        whitelist: vec![owner_addr.to_string(), caller_addr.to_string()],
        token_a: TokenData {
            denom: "untrn".to_string(),
            decimals: 6,
            max_blocks_old: 100,
            pair: CurrencyPair {
                base: "untrn".to_string(),
                quote: "usd".to_string(),
            },
        },
        token_b: TokenData {
            denom: "usd".to_string(),
            decimals: 6,
            max_blocks_old: 100,
            pair: CurrencyPair {
                base: "usd".to_string(),
                quote: "untrn".to_string(),
            },
        },
        fee_tier_config: FeeTierConfig {
            fee_tiers: vec![FeeTier { fee: 0, percentage: 0 }],
        },
        deposit_cap: Uint128::new(1000000),
        timestamp_stale: 1000,
        paused: false,
        oracle_contract: deps.api.addr_make("neutron1e2c5p8y5rw2hp4fjr05uvkrkz76ej0kqegnwxe").to_string(),
    };

    // Call the instantiate function
    instantiate(deps.as_mut(), env.clone(), info, instantiate_msg.clone())
        .expect("contract initialization should succeed");

    // Verify that the stored config matches the input data
    let config = CONFIG.load(&deps.storage).expect("config must be saved");

    // Convert Vec<Addr> to Vec<String> for comparison
    let whitelist_as_strings: Vec<String> = config.whitelist.iter().map(|addr| addr.to_string()).collect();

    assert_eq!(whitelist_as_strings, instantiate_msg.whitelist);
    assert_eq!(config.pair_data.token_0, instantiate_msg.token_a);
    assert_eq!(config.pair_data.token_1, instantiate_msg.token_b);
    assert_eq!(config.fee_tier_config, instantiate_msg.fee_tier_config);
    assert_eq!(config.deposit_cap, instantiate_msg.deposit_cap);
    assert_eq!(config.timestamp_stale, instantiate_msg.timestamp_stale);
    assert_eq!(config.paused, instantiate_msg.paused);
    assert_eq!(config.oracle_contract.to_string(), instantiate_msg.oracle_contract);
}

fn setup_test() -> (
    OwnedDeps<MockStorage, MockApi, WasmMockQuerier, Empty>,
    Env,
    MessageInfo
) {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = message_info(&deps.api.addr_make("creator"), &[]);

    // Set up oracle contract and prices
    let oracle_addr = deps.api.addr_make("cosmos1oracle");
    deps.querier.register_oracle(oracle_addr.clone());
    deps.querier.set_price(
        "untrn",
        PrecDec::from_ratio(25u128, 100u128)    // NTRN price in USD (2.5)
    );
    deps.querier.set_price(
        "uibcusdc",
        PrecDec::from_ratio(10u128, 10u128)    // USD price in NTRN (1.0)
    );

    // Set up owner
    let owner = deps.api.addr_make("cosmos1owner");

    // Instantiate contract
    let instantiate_msg = InstantiateMsg {
        whitelist: vec![owner.to_string()],
        token_a: TokenData {
            denom: "untrn".to_string(),
            decimals: 6,
            max_blocks_old: 100,
            pair: CurrencyPair {
                base: "NTRN".to_string(),
                quote: "USD".to_string(),
            },
        },
        token_b: TokenData {
            denom: "uibcusdc".to_string(),
            decimals: 6,
            max_blocks_old: 100,
            pair: CurrencyPair {
                base: "USDC".to_string(),
                quote: "USD".to_string(),
            },
        },
        fee_tier_config: FeeTierConfig {
            fee_tiers: vec![FeeTier { fee: 0, percentage: 0 }],
        },
        deposit_cap: Uint128::new(1000000),
        timestamp_stale: 1000,
        paused: false,
        oracle_contract: oracle_addr.to_string(),
    };

    instantiate(deps.as_mut(), env.clone(), info.clone(), instantiate_msg).unwrap();

    // Call CreateToken with the owner (who is whitelisted)
    let create_token_msg = ExecuteMsg::CreateToken {};
    execute(
        deps.as_mut(),
        env.clone(),
        message_info(&owner, &[]),
        create_token_msg,
    ).unwrap();
    
    // Create a binary response that matches what the contract expects
    let new_token_denom = format!("factory/{}/untrn-uibcusdc", env.contract.address);
    
    // Create a MsgCreateDenomResponse
    let token_creation_response = MsgCreateDenomResponse {
        new_token_denom: new_token_denom.clone(),
    };
    
    // Encode the response as a protobuf message
    let encoded_response = token_creation_response.encode_to_vec();
    
    // Mock the reply from token creation
    let reply_msg = Reply {
        id: CREATE_TOKEN_REPLY_ID,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            msg_responses: vec![cosmwasm_std::MsgResponse {
                type_url: "/osmosis.tokenfactory.v1beta1.MsgCreateDenomResponse".to_string(),
                value: Binary::from(encoded_response),
            }],
            data: None,
        }),
        gas_used: 0,
        payload: Binary::default(),
    };

    // Handle the reply
    reply(deps.as_mut(), env.clone(), reply_msg).unwrap();

    // Now check that the token was created via the config
    let config = CONFIG.load(&deps.storage).unwrap();
    let expected_lp_denom = format!("factory/{}/untrn-uibcusdc", env.contract.address);
    assert_eq!(config.lp_denom, expected_lp_denom);
    
    (deps, env, info)
}

#[test]
fn test_deposit() {
    let (mut deps, env, _info) = setup_test();
    let owner = deps.api.addr_make("cosmos1owner");
    let deposit_amount = Uint128::new(1000000);
    // First query prices to verify oracle is working
    let prices: CombinedPriceResponse = from_binary(
        &query(deps.as_ref(), env.clone(), QueryMsg::GetPrices {}).unwrap()
    ).unwrap();
    
    println!("Initial prices: {:?}", prices);  // Add debug output

    // Test deposit execution with funds
    let deposit_msg = ExecuteMsg::Deposit {};
    let funds = vec![
        cosmwasm_std::Coin {
            denom: "untrn".to_string(),
            amount: deposit_amount
        }
    ];

    let deposit_response = execute(
        deps.as_mut(),
        env,
        message_info(&owner, &funds),
        deposit_msg,
    ).expect("Deposit should succeed");



    // Verify that the response contains the expected attributes
    assert!(deposit_response.attributes.iter().any(|attr| attr.key == "action" && attr.value == "deposit"));
    assert!(deposit_response.attributes.iter().any(|attr| attr.key == "from"));
    assert!(deposit_response.attributes.iter().any(|attr| attr.key == "token_0_amount"));
    assert!(deposit_response.attributes.iter().any(|attr| attr.key == "token_1_amount"));
    assert!(deposit_response.attributes.iter().any(|attr| attr.key == "minted_amount"));
    //get the minted
    let empty_string = String::new();
    let minted_amount = deposit_response.attributes.iter()
        .find(|attr| attr.key == "minted_amount")
        .map(|attr| &attr.value)
        .unwrap_or(&empty_string);
    assert_eq!(minted_amount, "250000000000000");
    println!("Minted amount: {:?}", minted_amount);
}

#[test]
fn test_oracle_price_query() {
    let (deps, env, _info) = setup_test();

    // Query prices through the contract's query interface
    let prices: CombinedPriceResponse = from_binary(
        &query(deps.as_ref(), env, QueryMsg::GetPrices {}).unwrap()
    ).unwrap();
    let token_0_price_expected = PrecDec::from_ratio(10u128, 10u128);
    let token_1_price_expected = PrecDec::from_ratio(25u128, 100u128);
    let price_0_to_1_expected = token_0_price_expected.checked_div(token_1_price_expected).unwrap();
    assert_eq!(prices.token_0_price, token_0_price_expected);  // 2.5
    assert_eq!(prices.token_1_price, token_1_price_expected);  // 1.0
    assert_eq!(prices.price_0_to_1, price_0_to_1_expected);   // 0.4
}
