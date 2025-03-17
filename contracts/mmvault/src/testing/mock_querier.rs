use crate::msg::CombinedPriceResponse;
use cosmwasm_std::testing::{MockApi, MockStorage};
use cosmwasm_std::{
    from_json, to_binary, Coin, ContractResult, Empty, OwnedDeps, Querier,
    QuerierResult, QueryRequest, SystemError, SystemResult, WasmQuery, Binary, BankQuery, BalanceResponse, Uint128
};
use neutron_std::types::neutron::dex::{QueryAllUserDepositsResponse,  DepositRecord, MsgWithdrawalResponse};
use serde;
use std::collections::HashMap;
use prost::Message;

pub fn mock_dependencies_with_custom_querier(
    querier: MockQuerier,
) -> OwnedDeps<MockStorage, MockApi, MockQuerier, Empty> {
    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier,
        custom_query_type: std::marker::PhantomData,
    }
}

#[derive(Clone, Default)]
pub struct MockQuerier {
    price_response: Option<CombinedPriceResponse>,
    stale_price: bool,
    deposits: Option<QueryAllUserDepositsResponse>,
    contract_balances: HashMap<String, Vec<Coin>>,
    token_supply: HashMap<String, Uint128>,
    withdrawal_sim_response: Option<MsgWithdrawalResponse>,
}

impl MockQuerier {
    pub fn new() -> Self {
        MockQuerier::default()
    }

    pub fn set_price_response(&mut self, response: CombinedPriceResponse) {
        self.price_response = Some(response);
    }

    pub fn set_stale_price(&mut self, stale: bool) {
        self.stale_price = stale;
    }

    pub fn set_empty_deposits(&mut self) {
        self.deposits = Some(QueryAllUserDepositsResponse {
            deposits: vec![],
            pagination: None,
        });
    }

    pub fn set_deposits(&mut self, deposits: Vec<DepositRecord>) {
        self.deposits = Some(QueryAllUserDepositsResponse {
            deposits,
            pagination: None,
        });
    }

    pub fn set_contract_balance(&mut self, contract_addr: &str, balances: Vec<Coin>) {
        self.contract_balances.insert(contract_addr.to_string(), balances);
    }

    pub fn set_user_deposits_all_response(&mut self, deposits: Vec<DepositRecord>) {
        self.deposits = Some(QueryAllUserDepositsResponse {
            deposits,
            pagination: None,
        });
    }

    pub fn set_supply(&mut self, denom: &str, amount: u128) {
        self.token_supply.insert(denom.to_string(), Uint128::new(amount));
    }

    pub fn set_withdrawal_simulation_response(&mut self, response: MsgWithdrawalResponse) {
        self.withdrawal_sim_response = Some(response);
    }
}

impl Querier for MockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<Empty> = match from_json(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };

        match request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                // Handle oracle price queries
                if contract_addr.contains("oracle") {
                    if let Some(price_response) = &self.price_response {
                        if self.stale_price {
                            // Return a response indicating stale price
                            return SystemResult::Ok(ContractResult::Ok(
                                to_binary(&serde_json::json!({
                                    "error": "StalePrice",
                                    "stale": true
                                }))
                                .unwrap(),
                            ));
                        } else {
                            return SystemResult::Ok(ContractResult::Ok(
                                to_binary(price_response).unwrap(),
                            ));
                        }
                    }
                }
                // Handle dex deposit queries
                else if String::from_utf8_lossy(msg.as_slice()).contains("user_deposits_all") {
                    if let Some(deposits) = &self.deposits {
                        return SystemResult::Ok(ContractResult::Ok(
                            to_binary(deposits).unwrap(),
                        ));
                    }
                }
                // Handle balance queries
                else if String::from_utf8_lossy(msg.as_slice()).contains("balance") {
                    if let Some(balances) = self.contract_balances.get(&contract_addr) {
                        return SystemResult::Ok(ContractResult::Ok(
                            to_binary(&balances).unwrap(),
                        ));
                    }
                }

                SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Unhandled smart query: {}", contract_addr),
                    request: bin_request.into(),
                })
            },
            QueryRequest::Grpc(grpc_query) => {
                let path = grpc_query.path;
                
                // Handle GRPC queries
                if path.contains("UserDepositsAll") {
                    if let Some(deposits) = &self.deposits {
                        // Encode the response as a protobuf message
                        let encoded = deposits.encode_to_vec();
                        return SystemResult::Ok(ContractResult::Ok(
                            Binary::from(encoded)
                        ));
                    }
                }
                
                SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Unhandled GRPC query: {}", path),
                    request: bin_request.into(),
                })
            },
            QueryRequest::Bank(BankQuery::Balance { address, denom }) => {
                // Check if we have a balance for this contract
                if let Some(balances) = self.contract_balances.get(&address) {
                    // Find the requested denom
                    let amount = balances
                        .iter()
                        .find(|c| c.denom == denom)
                        .map(|c| c.amount)
                        .unwrap_or_default();

                    let bank_response = BalanceResponse::new(Coin {
                        denom: denom.clone(),
                        amount
                    });
                    SystemResult::Ok(ContractResult::Ok(to_binary(&bank_response).unwrap()))
                } else {
                    // Return zero balance if not found
                    let bank_response = BalanceResponse::new(Coin {
                        denom: denom.clone(),
                        amount: Uint128::zero()
                    });
                    SystemResult::Ok(ContractResult::Ok(to_binary(&bank_response).unwrap()))
                }
            },
            QueryRequest::Bank(BankQuery::Supply { denom }) => {
                let amount = self.token_supply.get(&denom).cloned().unwrap_or_default();
                let supply_response = cosmwasm_std::SupplyResponse::new (
                    Coin {
                        denom: denom.clone(),
                        amount,
                    },
            );
                SystemResult::Ok(ContractResult::Ok(to_binary(&supply_response).unwrap()))
            },
            _ => SystemResult::Err(SystemError::InvalidRequest {
                error: "Unhandled query".to_string(),
                request: bin_request.into(),
            }),
        }
    }
}