use alloy::primitives::B256;
use alloy::signers::Signer;
use alloy::signers::local::PrivateKeySigner;
use ferrofluid::types::actions::Agent;
use ferrofluid::types::eip712::HyperliquidAction;

#[tokio::main]
async fn main() {
    // API wallet private key
    let key_hex = "0bb028174905a030304d5773b50a4dc15f61052e2e1b46fa98041d3f2f875935";
    let key_bytes = hex::decode(key_hex).unwrap();
    let signer = PrivateKeySigner::from_bytes(&B256::from_slice(&key_bytes)).unwrap();

    println!("Signer address: {:?}", signer.address());

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
    let signing_hash = agent.eip712_signing_hash(&domain);

    println!("Signing hash: 0x{}", hex::encode(signing_hash));

    // Sign the hash
    let sig = signer.sign_hash(&signing_hash).await.unwrap();

    println!("Signature r: 0x{:064x}", sig.r());
    println!("Signature s: 0x{:064x}", sig.s());
    println!("Signature v: {}", sig.v());

    // Now recover the address from the signature
    let recovered = sig.recover_address_from_prehash(&signing_hash).unwrap();
    println!("Recovered address: {:?}", recovered);
    println!("Expected address: {:?}", signer.address());
    println!("Match: {}", recovered == signer.address());
}
