use alloy::primitives::B256;
use ferrofluid::types::actions::Agent;
use ferrofluid::types::eip712::HyperliquidAction;

fn main() {
    // Connection ID from our actual test
    let connection_id = B256::from_slice(
        &hex::decode("739f0c0fd9b2867dde2c7d4a6f982f167811d9172a8cd38ee8f1fc58aff58571")
            .unwrap(),
    );

    let agent = Agent {
        source: "b".to_string(),
        connection_id,
    };

    let domain = agent.domain();

    // Print domain info
    println!("Domain separator: 0x{}", hex::encode(domain.separator()));

    // Print type hash
    let type_hash = Agent::type_hash();
    println!("Type hash: 0x{}", hex::encode(type_hash));

    // Print struct hash
    let struct_hash = agent.struct_hash();
    println!("Struct hash: 0x{}", hex::encode(struct_hash));

    // Print signing hash
    let signing_hash = agent.eip712_signing_hash(&domain);
    println!("Signing hash: 0x{}", hex::encode(signing_hash));
}
