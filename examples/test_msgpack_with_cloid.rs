use ferrofluid::types::requests::{BuilderInfo, OrderRequest};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BulkOrder {
    pub orders: Vec<OrderRequest>,
    pub grouping: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builder: Option<BuilderInfo>,
}

fn main() {
    // Order WITH cloid like ferrofluid uses
    let order_with_cloid = OrderRequest::limit(3, true, "84511", "0.00014", "Gtc")
        .with_cloid(Some(uuid::Uuid::from_u128(0x425)));

    // Order WITHOUT cloid like Python SDK
    let order_without_cloid = OrderRequest::limit(3, true, "84511", "0.00014", "Gtc");

    let bulk_with = BulkOrder {
        orders: vec![order_with_cloid],
        grouping: "na".to_string(),
        builder: None,
    };

    let bulk_without = BulkOrder {
        orders: vec![order_without_cloid],
        grouping: "na".to_string(),
        builder: None,
    };

    // Create action with type field
    #[derive(Serialize)]
    #[serde(tag = "type")]
    #[serde(rename_all = "camelCase")]
    enum ActionWrapper {
        Order(BulkOrder),
    }

    let wrapped_with = ActionWrapper::Order(bulk_with.clone());
    let wrapped_without = ActionWrapper::Order(bulk_without.clone());

    let msgpack_with = rmp_serde::to_vec_named(&wrapped_with).unwrap();
    let msgpack_without = rmp_serde::to_vec_named(&wrapped_without).unwrap();

    println!("WITH cloid:");
    println!("  JSON: {}", serde_json::to_string(&bulk_with).unwrap());
    println!("  MessagePack hex: {}", hex::encode(&msgpack_with));
    println!("  Length: {}", msgpack_with.len());

    println!("\nWITHOUT cloid:");
    println!("  JSON: {}", serde_json::to_string(&bulk_without).unwrap());
    println!("  MessagePack hex: {}", hex::encode(&msgpack_without));
    println!("  Length: {}", msgpack_without.len());

    // Also compare hashes
    let nonce: u64 = 1764971519874;

    let mut bytes_with = msgpack_with;
    bytes_with.extend(nonce.to_be_bytes());
    bytes_with.push(0);
    let hash_with = alloy::primitives::keccak256(&bytes_with);
    println!("\nHash WITH cloid: {:?}", hash_with);

    let mut bytes_without = msgpack_without;
    bytes_without.extend(nonce.to_be_bytes());
    bytes_without.push(0);
    let hash_without = alloy::primitives::keccak256(&bytes_without);
    println!("Hash WITHOUT cloid: {:?}", hash_without);

    // Python's hash was: 0xb37328d1bbae010b23107cd0e9056d616169c7dabb68774b8cc77cc8d24d1349
    println!(
        "\nPython's hash: 0xb37328d1bbae010b23107cd0e9056d616169c7dabb68774b8cc77cc8d24d1349"
    );
}
