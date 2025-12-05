use alloy::primitives::B256;
use alloy::signers::local::PrivateKeySigner;

fn main() {
    let key_hex = "0bb028174905a030304d5773b50a4dc15f61052e2e1b46fa98041d3f2f875935";
    let key_bytes = hex::decode(key_hex).unwrap();
    let signer = PrivateKeySigner::from_bytes(&B256::from_slice(&key_bytes)).unwrap();

    println!("Signer address: {:?}", signer.address());
    println!("Expected: 0xC4452dD6C3CdCeac06156386f596e63285661080");
}
