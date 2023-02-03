use std::convert::TryFrom;

use std::sync::Arc;

use ethers::core::types::Address;
use ethers::providers::{Http, Provider};
use ethers::signers::Signer;
use eyre::WrapErr;
use namada_core::proto::Signable;
use namada_core::proto::SignableEthMessage;
use namada_core::proto::Signed;
use namada_core::types::ethereum_events::EthAddress;
use namada_core::types::keccak::keccak_hash;
use namada_core::types::key::{self, RefTo, SecretKey, SigScheme};

mod sign {
    ethers::contract::abigen!(
        Sign,
        "res/Sign.abi";
    );
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let client = Arc::new(
        Provider::<Http>::try_from("http://localhost:8545").wrap_err("Failed to get provider")?,
    );
    test_hashing(Arc::clone(&client)).await?;
    test_signing(Arc::clone(&client)).await?;
    test_signing_js(Arc::clone(&client)).await?;
    Ok(())
}

async fn test_hashing(client: Arc<Provider<Http>>) -> eyre::Result<()> {
    let sign_address = "0x9fe46736679d2d9a65f0992f2272de9f3c7fa6e0".parse::<Address>()?;
    let sign = sign::Sign::new(sign_address, client);
    let data_hash = keccak_hash(vec![0xff; 32]);
    let eth_msg_hash = SignableEthMessage::as_signable(&data_hash);
    let is_valid_hash = sign
        .is_valid_hash(data_hash.0, eth_msg_hash.0)
        .call()
        .await?;
    println!("valid hash? {is_valid_hash}");
    Ok(())
}

async fn test_signing(client: Arc<Provider<Http>>) -> eyre::Result<()> {
    let sign_address = "0x9fe46736679d2d9a65f0992f2272de9f3c7fa6e0".parse::<Address>()?;
    let sign = sign::Sign::new(sign_address, client);
    let (signer, hash, signature) = {
        let sk = gen_secp256k1_keypair();
        let addr: EthAddress = match sk.ref_to() {
            key::common::PublicKey::Secp256k1(ref k) => k.into(),
            _ => panic!("AAAAAAAAAA"),
        };
        let signed: Signed<_, SignableEthMessage> = Signed::new(&sk, keccak_hash(b"hi"));
        let key::common::Signature::Secp256k1(sig) = signed.sig else {
            panic!("AAAAAA");
        };
        let (r, s, v) = sig.into_eth_rsv();
        (
            ethers::types::H160(addr.0),
            signed.data.0,
            sign::Signature { r, s, v },
        )
    };
    let is_valid_sig = sign
        .is_valid_signature(signer, hash, signature)
        .call()
        .await?;
    println!("valid signature? {is_valid_sig}");
    Ok(())
}

/// Generate a random [`key::secp256k1`] keypair.
pub fn gen_secp256k1_keypair() -> key::common::SecretKey {
    use rand::rngs::ThreadRng;
    use rand::thread_rng;
    let mut rng: ThreadRng = thread_rng();
    key::secp256k1::SigScheme::generate(&mut rng)
        .try_to_sk()
        .unwrap()
}

async fn test_signing_js(client: Arc<Provider<Http>>) -> eyre::Result<()> {
    let sign_address = "0x9fe46736679d2d9a65f0992f2272de9f3c7fa6e0".parse::<Address>()?;
    let sign = sign::Sign::new(sign_address, client);
    let (signature, data_hash, addr) = sign_message().await;
    let is_valid_sig = sign
        .is_valid_signature(
            addr,
            data_hash,
            sign::Signature {
                r: signature.r.into(),
                s: signature.s.into(),
                v: signature.v as u8,
            },
        )
        .call()
        .await?;
    println!("valid signature (ethers)? {is_valid_sig}");
    Ok(())
}

async fn sign_message() -> (ethers::types::Signature, [u8; 32], ethers::types::H160) {
    // instantiate the wallet
    let wallet = "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7"
        .parse::<ethers::prelude::LocalWallet>()
        .unwrap();

    // can also sign a message
    let data_hash = keccak_hash(b"hi");
    let eth_msg_hash = ethers::types::H256(SignableEthMessage::as_signable(&data_hash).0);
    let signature = wallet.sign_hash(eth_msg_hash);
    let recovered_addr = signature.recover(eth_msg_hash).unwrap();
    assert_eq!(recovered_addr, wallet.address());
    (signature, data_hash.0, wallet.address())
}
