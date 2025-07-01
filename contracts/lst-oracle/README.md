# LST Oracle Contract

## Overview

The LST (Liquid Staked Token) oracle provides pricing for liquid staked assets to the vault that integrates it. It uses Slinky to fetch underlying asset prices and applies configurable redemption rates to calculate the value of liquid staked token.

It will return the price of the non-LST asset (eg: BTC) the price of the LST (eg: someBTC) as well as their ratio for the vault to use.

## Integration Flow
![Integration Flow](../docs/images/lst_flow.png)

## Key Features

- **Liquid Staked Token Pricing**: Get the price for the LST by applying a configurable redemption rate to underlying asset price
- **Dual Token Price Queries**: Retrieve prices for two tokens simultaneously for the vault to use
- **Slinky Oracle**: Use Slinky oracle as the main price feed
- **Price Validation**: Ensure base prices are recent and non-nil

## Security Considerations

- **Owner Controls**: The contract owner can update the redemption rate and LST asset denomination
- **Oracle Dependency**: Relies on Slinky oracle availability and accuracy
- **Redemption Rate Updates**: Manual updates required when redemption rate changes


## How It Works

1. **Price Queryy**: The contract queries the Slinky oracle for the underlying asset prices (e.g., LST/USDC)
2. **Redemption Rate Application**: If one of the tokens is the configured LST asset, the contract multiplies its price by the redemption rate
3. **Validation**: Ensures prices are:
   - Recent (within `max_blocks_old` blocks)
   - Non-nil (valid price data exists)
   - From supported markets (available in Slinky oracle and marketmap)
4. **Price Calculation**: Returns individual prices and their ratio for the vault's convenience


## Contract Interface

### Query Messages

#### GetPrices
Retrieves prices for two tokens and their exchange ratio.

```json
{
  "get_prices": {
    "token_a": {
      "denom": "DENOM_FOR_BASE_ASSET",
      "decimals": 6,
      "pair": {
        "base": "BTC",
        "quote": "USD"
      },
      "max_blocks_old": 100
    },
    "token_b": {
      "denom": "DENOM_FOR_LST",
      "decimals": 6,
      "pair": {
        "base": "BTC",
        "quote": "USD"
      },
      "max_blocks_old": 100
    }
  }
}
```

**Response:**
```json
{
  "token_0_price": "100000.00000000000000",
  "token_1_price": "105000.00000000000000",
  "price_0_to_1": "0.9523809542381"
}
```

#### GetRedemptionRate
Retrieves the current redemption rate for the LST asset.

```json
{
  "get_redemption_rate": {}
}
```

**Response:**
```json
"1.054000000000000000"
```

### Execute Messages (Owner Only)

#### UpdateConfig
Updates the LST asset denomination and/or redemption rate.

```json
{
  "update_config": {
    "new_config": {
      "lst_asset_denom": "BTC_LST_DENOM",
      "redemption_rate": "1.054000000000000000"
    }
  }
}
```

## Data Structures

### TokenData
```json
{
  "denom": "string",           // The token denomination (e.g., "uatom", "stATOM")
  "decimals": 6,               // Number of decimal places for the token
  "pair": {
    "base": "ATOM",            // Base currency symbol for Slinky oracle
    "quote": "USD"             // Quote currency (USD or USDC supported)
  },
  "max_blocks_old": 100        // Maximum age in blocks for price data
}
```

### CombinedPriceResponse
```json
{
  "token_0_price": "12.450000000000000000",    // Price of token_a in USD
  "token_1_price": "13.122500000000000000",    // Price of token_b in USD (with redemption rate applied if LST)
  "price_0_to_1": "0.948764461832061068"       // Exchange ratio: token_0_price / token_1_price
}

```
### CLI Query
```bash
# Query prices
neutrond q wasm contract-state smart [CONTRACT_ADDRESS] \
  '{"get_prices":{"token_a":{"denom":"uatom","decimals":6,"pair":{"base":"ATOM","quote":"USD"},"max_blocks_old":100},"token_b":{"denom":"dATOM","decimals":6,"pair":{"base":"ATOM","quote":"USD"},"max_blocks_old":100}}}' \
  --node https://rpc.neutron.org

# Query redemption rate
neutrond q wasm contract-state smart [CONTRACT_ADDRESS] \
  '{"get_redemption_rate":{}}' \
  --node https://rpc.neutron.org
```



## Error Handling

The contract validates several conditions and will return an error if:

- **UnsupportedMarket**: The requested currency pair is not available in Slinky oracle or marketmap
- **PriceTooOld**: The price data is older than the specified `max_blocks_old`
- **PriceIsNil**: No valid price data is available for the currency pair
- **PriceNotAvailable**: The oracle did not return price data
- **InvalidPrice**: The price format is invalid or negative

