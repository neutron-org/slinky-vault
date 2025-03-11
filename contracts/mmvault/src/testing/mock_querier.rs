use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{
    from_binary, to_binary, Binary, ContractResult, Empty, OwnedDeps, Querier, QuerierResult,
    QueryRequest, SystemError, SystemResult, WasmQuery,
};
use neutron_std::types::neutron::util::precdec::PrecDec;
use std::collections::HashMap;
use std::marker::PhantomData;
use crate::msg::{CombinedPriceResponse};
use serde_json::Value;
use neutron_std::types::neutron::dex::{QueryAllUserDepositsResponse};
use prost::Message;

pub struct WasmMockQuerier {
    base: MockQuerier,
    price_data: HashMap<String, PrecDec>,
    oracle_contract: String,
}

impl WasmMockQuerier {
    pub fn new() -> Self {
        WasmMockQuerier {
            base: MockQuerier::new(&[]),
            price_data: HashMap::new(),
            oracle_contract: "oracle_contract_addr".to_string(),
        }
    }

    pub fn register_oracle(&mut self, address: impl Into<String>) {
        self.oracle_contract = address.into();
    }

    pub fn set_price(&mut self, token: &str, price: PrecDec) {
        self.price_data.insert(token.to_string(), price);
    }

    fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                if contract_addr == &self.oracle_contract {
                    let parsed: Value = match from_binary(msg) {
                        Ok(parsed_msg) => parsed_msg,
                        Err(e) => {
                            return SystemResult::Err(SystemError::InvalidRequest {
                                error: format!("Parsing query request: {}", e),
                                request: msg.clone(),
                            });
                        }
                    };
                    
                    if parsed.get("get_prices").is_some() {
                        let token_0_price = self.price_data.get("uibcusdc")
                            .cloned()
                            .unwrap_or_else(PrecDec::one);
                        let token_1_price = self.price_data.get("untrn")
                            .cloned()
                            .unwrap_or_else(PrecDec::one);

                        let price_0_to_1 = token_0_price / token_1_price;

                        let response = CombinedPriceResponse {
                            token_0_price,
                            token_1_price,
                            price_0_to_1,
                        };

                        match to_binary(&response) {
                            Ok(binary_response) => SystemResult::Ok(ContractResult::Ok(binary_response)),
                            Err(e) => SystemResult::Err(SystemError::InvalidResponse {
                                error: format!("Serializing response: {}", e),
                                response: vec![].into(),
                            }),
                        }
                    } else if parsed.get("user_deposits_all").is_some() {
                        // Handle DexQuerier::user_deposits_all query
                        let response = QueryAllUserDepositsResponse {
                            deposits: vec![],
                            pagination: None,
                        };
                        
                        match to_binary(&response) {
                            Ok(binary_response) => SystemResult::Ok(ContractResult::Ok(binary_response)),
                            Err(e) => SystemResult::Err(SystemError::InvalidResponse {
                                error: format!("Serializing response: {}", e),
                                response: vec![].into(),
                            }),
                        }
                    } else {
                        self.base.handle_query(request)
                    }
                } else if contract_addr.to_string().contains("neutron") {
                    // This is likely a DexQuerier query
                    let parsed: Value = match from_binary(msg) {
                        Ok(parsed_msg) => parsed_msg,
                        Err(e) => {
                            return SystemResult::Err(SystemError::InvalidRequest {
                                error: format!("Parsing query request: {}", e),
                                request: msg.clone(),
                            });
                        }
                    };
                    
                    if parsed.get("user_deposits_all").is_some() {
                        // Handle DexQuerier::user_deposits_all query
                        let response = QueryAllUserDepositsResponse {
                            deposits: vec![],
                            pagination: None,
                        };
                        
                        match to_binary(&response) {
                            Ok(binary_response) => SystemResult::Ok(ContractResult::Ok(binary_response)),
                            Err(e) => SystemResult::Err(SystemError::InvalidResponse {
                                error: format!("Serializing response: {}", e),
                                response: vec![].into(),
                            }),
                        }
                    } else {
                        self.base.handle_query(request)
                    }
                } else {
                    self.base.handle_query(request)
                }
            },
            QueryRequest::Grpc(_) => {
                // Create a properly encoded protobuf response
                // For DexAllUserDeposits, return an empty list
                let response = QueryAllUserDepositsResponse {
                    deposits: vec![],
                    pagination: None,
                };
                
                // Encode the response as a protobuf message
                let encoded = response.encode_to_vec();
                
                // Return the encoded response
                SystemResult::Ok(ContractResult::Ok(Binary::from(encoded)))
            },
            _ => self.base.handle_query(request),
        }
    }
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<Empty> = match from_binary(&Binary::from(bin_request)) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: Binary::from(bin_request),
                });
            }
        };
        self.handle_query(&request)
    }
}

pub fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier, Empty> {
    let custom_storage = MockStorage::default();
    let custom_querier = WasmMockQuerier::new();

    OwnedDeps {
        storage: custom_storage,
        api: MockApi::default(),
        querier: custom_querier,
        custom_query_type: PhantomData,
    }
}
