use std::convert::TryFrom;

use std::sync::Arc;

use ethers::core::types::Address;
use ethers::providers::{Http, Provider};
use eyre::WrapErr;

// We use `abigen!()` to generate bindings automatically
// for smart contract calls. This is a pretty useful procmacro.
mod bridge {
    ethers::contract::abigen!(
        Bridge,
        "res/Bridge.abi";
    );
}

mod test_erc_20 {
    ethers::contract::abigen!(
        TestERC20,
        "res/TestERC20.abi";
    );
}

mod governance {
    ethers::contract::abigen!(
        Governance,
        "res/Governance.abi";
    );
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Connect to the network. We are assuming that a `hardhat` node is running in the
    // background, with some contracts already deployed.
    //
    // <https://github.com/sug0/ethereum-bridge/tree/tiago/test/deploy-contracts>
    let client = Arc::new(
        Provider::<Http>::try_from("http://localhost:8545").wrap_err("Failed to get provider")?,
    );

    test_abigen_current_val_set(Arc::clone(&client)).await?;
    test_abigen_current_nonce(Arc::clone(&client)).await?;

    Ok(())
}

/// Read the current validator set. The contract is initialized from a JSON file
/// read at compile-time.
async fn test_abigen_current_val_set(client: Arc<Provider<Http>>) -> eyre::Result<()> {
    let bridge_address = "0x5FC8d32690cc91D4c39d9d3abcBD16989F875707".parse::<Address>()?;
    let bridge = bridge::Bridge::new(bridge_address, client);
    // The method `.call()` is used for read-only calls. Therefore, it does
    // not need to be sent as a tx, and does not need block confirmations.
    let valset = bridge.current_validator_set_hash().call().await?;
    println!("{valset:?}");
    Ok(())
}

/// Read the current nonce.
async fn test_abigen_current_nonce(client: Arc<Provider<Http>>) -> eyre::Result<()> {
    let governance_address = "0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9".parse::<Address>()?;
    let governance = governance::Governance::new(governance_address, client);
    // The method `.call()` is used for read-only calls. Therefore, it does
    // not need to be sent as a tx, and does not need block confirmations.
    let nonce = governance.validator_set_nonce().call().await?;
    println!("{nonce}");
    Ok(())
}
