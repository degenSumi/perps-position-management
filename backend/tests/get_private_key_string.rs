use solana_sdk::signature::{Keypair};

#[test]
fn main() {
    let bytes = []; // Replace with your 64-byte array
    let key = Keypair::from_bytes(&bytes).unwrap();
    println!("Base58 Private Key:");
    println!("{}", key.to_base58_string());
    
}