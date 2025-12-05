use ferrofluid::l1_action;
use ferrofluid::types::requests::{BuilderInfo, Limit, OrderRequest, OrderType};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BulkOrder {
    pub orders: Vec<OrderRequest>,
    pub grouping: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builder: Option<BuilderInfo>,
}

fn main() {
    let order = OrderRequest::limit(3, false, "93515", "0.00013", "Gtc")
        .with_cloid(Some(uuid::Uuid::from_u128(0x426)));

    let bulk_order = BulkOrder {
        orders: vec![order],
        grouping: "na".to_string(),
        builder: None,
    };

    // Test 1: Serialize BulkOrder as JSON
    println!("JSON: {}", serde_json::to_string(&bulk_order).unwrap());

    // Test 2: Create action with type field
    #[derive(Serialize)]
    #[serde(tag = "type")]
    #[serde(rename_all = "camelCase")]
    enum ActionWrapper {
        Order(BulkOrder),
    }

    let wrapped = ActionWrapper::Order(bulk_order.clone());
    let msgpack_bytes = rmp_serde::to_vec_named(&wrapped).unwrap();
    println!("MessagePack (hex): {}", hex::encode(&msgpack_bytes));
    println!("MessagePack length: {}", msgpack_bytes.len());

    // Now compute the full hash as done in Python SDK
    let nonce: u64 = 1764970752346;
    let mut bytes = msgpack_bytes;
    bytes.extend(nonce.to_be_bytes());
    bytes.push(0); // no vault address

    let hash = alloy::primitives::keccak256(&bytes);
    println!("Connection ID (hash): {:?}", hash);
}
