use std::convert::TryFrom;
use std::fs;
use std::sync::Arc;

use ethers::abi::FixedBytes;
use ethers::contract::Contract;
use ethers::core::{abi::Abi, types::Address};
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
    test_abigen_transfer_eth_to_nam(Arc::clone(&client)).await?;
    test_runtime_current_val_set(
        Arc::try_unwrap(client).map_err(|_| eyre::eyre!("Failed to unwrap client Arc"))?,
    )
    .await?;

    Ok(())
}

/// Read the current validator set. The contract is initialized from a JSON file
/// read at runtime.
async fn test_runtime_current_val_set(client: Provider<Http>) -> eyre::Result<()> {
    let bridge_address = "0x5FC8d32690cc91D4c39d9d3abcBD16989F875707".parse::<Address>()?;
    let bridge_abi: Abi = serde_json::from_str(&{
        let path = "res/Bridge.abi";
        fs::read_to_string(path).wrap_err_with(|| format!("Failed to read file path: {path}"))?
    })
    .wrap_err("Failed to parse json")?;

    // Instantiate a contract at runtime, from the provided
    // parsed ABI.
    let contract = Contract::new(bridge_address, bridge_abi, client);

    // Call the contract. We do the type checking manually.
    let valset = contract
        .method::<_, FixedBytes>("currentValidatorSetHash", ())?
        .call()
        .await
        .wrap_err("Failed to get currentValidatorSetHash")?;

    println!("{valset:?}");
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

/// Perform a transfor of some TestERC20 tokens to an arbitrary Namada address.
async fn test_abigen_transfer_eth_to_nam(client: Arc<Provider<Http>>) -> eyre::Result<()> {
    let bridge_address = "0x5FC8d32690cc91D4c39d9d3abcBD16989F875707".parse::<Address>()?;
    let test_erc20_address = "0x5FbDB2315678afecb367f032d93F642f64180aa3".parse::<Address>()?;

    let bridge = bridge::Bridge::new(bridge_address.clone(), Arc::clone(&client));
    let test_erc20 = test_erc_20::TestERC20::new(test_erc20_address.clone(), client);

    let approve_result = test_erc20.approve(bridge_address, 100.into()).await?;

    if !approve_result {
        return Err(eyre::eyre!("TestERC20 transfer tx not approved"));
    }

    let transfers = vec![bridge::NamadaTransfer {
        from: test_erc20_address,
        to: "atest1v4ehgw36xuunwd6989prwdfkxqmnvsfjxs6nvv6xxucrs3f3xcmns3fcxdzrvvz9xverzvzr56le8f"
            .into(),
        amount: 100.into(),
    }];

    let transf_result_call = bridge.transfer_to_namada(transfers, 1.into());
    let pending_tx = transf_result_call.send().await?;

    // The method `.send()` is used for mutable calls. For this reason, it
    // needs a certain number of block confirmations.
    let transf_result = pending_tx.confirmations(3).await?;
    println!("{transf_result:?}");

    Ok(())
}
