use std::convert::TryFrom;

use std::sync::Arc;

use ethers::core::types::Address;
use ethers::providers::{Http, Provider};
use eyre::WrapErr;
use namada_core::proto::Signable;
use namada_core::proto::SignableEthBytes;
use namada_core::proto::Signed;
use namada_core::types::ethereum_events::EthAddress;
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
    test_signing(Arc::clone(&client)).await?;
    Ok(())
}

async fn test_signing(client: Arc<Provider<Http>>) -> eyre::Result<()> {
    let sign_address = "0x9fe46736679d2d9a65f0992f2272de9f3c7fa6e0".parse::<Address>()?;
    let sign = sign::Sign::new(sign_address, client);
    let (signer, hash, signature) = {
        let sk = gen_secp256k1_keypair();
        // XXX: the conversion from pubkey to ethaddr could be faulty
        let addr: EthAddress = match sk.ref_to() {
            key::common::PublicKey::Secp256k1(ref k) => k.into(),
            _ => panic!("AAAAAAAAAA"),
        };
        let signed: Signed<_, SignableEthBytes> = Signed::new(&sk, b"hi");
        let key::common::Signature::Secp256k1(sig) = signed.sig else {
            panic!("AAAAAA");
        };
        let r = sig.0.r.b32();
        let s = sig.0.s.b32();
        let v = sig.1.serialize();
        (
            ethers::types::H160(addr.0),
            SignableEthBytes::as_signable(signed.data).0,
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
