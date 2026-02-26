# MaxBTC Oracle Contract

This contract provides oracle functionality for both maxBTC and LST (Liquid Staking Token) assets, returning redemption rate prices for both types of tokens.

## Features

- **Dual Oracle Functionality**: Supports both maxBTC and LST tokens with their respective redemption rates
- **MaxBTC Dynamic Rates**: Queries maxBTC exchange rates from the maxBTC core contract
- **LST Manual Rates**: Stores and allows updates to LST redemption rates
- **Price Calculation**: Combines Slinky oracle prices with redemption rates for accurate pricing
- **Multi-Owner Management**: Uses multiple owner addresses stored in config for secure admin operations

## Contract Structure

### State
- `owners`: Vector of authorized addresses that can perform admin operations
- `maxbtc_core_contract`: Address of the maxBTC core contract for querying exchange rates
- `maxbtc_denom`: Token denomination for maxBTC
- `lst_denom`: Token denomination for the LST token
- `lst_redemption_rate`: Manually set redemption rate for the LST token

### Key Functions

#### Instantiation
```rust
pub struct InstantiateMsg {
    pub initial_owners: Vec<String>,
    pub maxbtc_core_contract: String,
    pub maxbtc_denom: String,
    pub lst_denom: String,
    pub lst_redemption_rate: PrecDec,
}
```

#### Queries
- `GetPrices { token_a, token_b }`: Returns combined price data for two tokens with redemption rates applied
- `GetRedemptionRates {}`: Returns both maxBTC and LST redemption rates
- `GetMaxBtcRedemptionRate {}`: Returns only maxBTC redemption rate
- `GetLstRedemptionRate {}`: Returns only LST redemption rate
- `GetMaxBtcDenom {}`: Returns maxBTC token denomination
- `GetLstDenom {}`: Returns LST token denomination
- `GetOwners {}`: Returns list of all owner addresses
- `IsOwner { address }`: Checks if a specific address is an owner

#### Execute Messages
- `UpdateConfig`: Allows any owner to update contract configuration including owners list, redemption rates, and other settings

## MaxBTC Integration

The contract queries the maxBTC core contract using:
```rust
#[cw_serde]
pub enum CoreQueryMsg {
    ExchangeRate {},
}
```

This returns a `Decimal` value representing the current exchange rate.

## Price Calculation Logic

1. **Base Price Retrieval**: Gets Slinky oracle prices for the requested token pairs
2. **Token Identification**: Determines if tokens are maxBTC, LST, or other assets
3. **Redemption Rate Application**: 
   - For maxBTC tokens: Queries live exchange rate from core contract
   - For LST tokens: Uses stored redemption rate from config
   - For other tokens: Uses base price only
4. **Price Normalization**: Adjusts for token decimals
5. **Ratio Calculation**: Computes price ratios between token pairs

## Configuration Updates

The contract owner can update:
- MaxBTC core contract address
- MaxBTC token denomination
- LST token denomination  
- LST redemption rate (for easy manual updates)

## Error Handling

Comprehensive error handling for:
- Invalid oracle prices
- Market validation failures
- Conversion errors
- Access control violations
- Contract query failures

## Dependencies

- CosmWasm standard library
- Neutron SDK for Slinky oracle integration
- cw-ownable for ownership management
- PrecDec for high-precision decimal operations
