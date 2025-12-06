# Fix: Client Order ID (cloid) Format

## Issue

Orders with client order IDs (cloid) were failing with signature verification errors:

```
User or API Wallet 0x... does not exist
```

The root cause was that the cloid was being serialized in an incorrect format, causing the MessagePack hash used for signing to differ from what Hyperliquid's server expected.

## Root Cause

The cloid format in ferrofluid did not match the official Hyperliquid SDKs:

| SDK | Format | Example |
|-----|--------|---------|
| **ferrofluid (wrong)** | `{:032x}` from `uuid.as_u128()` | `1e60610f0b3d420597c88c1fed2ad5ee` |
| **Official Rust SDK** | `0x` + hex from `uuid.as_bytes()` | `0x1e60610f0b3d420597c88c1fed2ad5ee` |
| **Python SDK** | `0x` + hex from UUID bytes | `0x1e60610f0b3d420597c88c1fed2ad5ee` |

The missing `0x` prefix caused the MessagePack serialization to produce a different hash, resulting in an invalid signature.

## Fix

Added `uuid_to_hex_string()` helper function in `src/types/requests.rs` that matches the official SDK format:

```rust
/// Convert UUID to hex string with 0x prefix, matching official SDK format
/// Uses UUID bytes directly to ensure correct byte ordering
pub fn uuid_to_hex_string(uuid: Uuid) -> String {
    let hex_string = uuid
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<String>>()
        .join("");
    format!("0x{hex_string}")
}
```

Updated all places that format cloid:
- `OrderRequest::with_cloid()`
- `CancelRequestCloid::new()`
- `OrderBuilder::build()`

## Verification

After the fix, orders with cloid submit successfully:

```
Submitting order: asset=4 (ETH), is_buy=true, px=2887.5, sz=0.0042, cloid=00000000-0000-0000-0000-000000000425
SDK response: Ok(ExchangeResponse { statuses: [Resting(RestingOrder { oid: 44553386266 })] })
```

## Files Changed

- `src/types/requests.rs` - Added `uuid_to_hex_string()`, updated `with_cloid()` and `CancelRequestCloid::new()`
- `src/providers/exchange.rs` - Updated `OrderBuilder::build()` to use the new helper

## References

- Official Rust SDK: https://github.com/hyperliquid-dex/hyperliquid-rust-sdk/blob/master/src/helpers.rs
- Python SDK: https://github.com/hyperliquid-dex/hyperliquid-python-sdk/blob/master/hyperliquid/utils/signing.py
