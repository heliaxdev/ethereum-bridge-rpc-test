mod contracts;
mod namada_queries;

use std::convert::TryFrom;

use std::sync::Arc;

use ethers::core::types::Address;
use ethers::providers::{Http, Provider};
use eyre::WrapErr;

use self::contracts::{bridge, test_erc20};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Connect to the network. We are assuming that a `hardhat` node is running in the
    // background, with some contracts already deployed.
    //
    // <https://github.com/sug0/ethereum-bridge/tree/tiago/test/deploy-contracts>
    let client = Arc::new(
        Provider::<Http>::try_from("http://localhost:8545").wrap_err("Failed to get provider")?,
    );

    test_abigen_transfer_eth_to_nam(Arc::clone(&client)).await?;

    Ok(())
}

/// Perform a transfor of some TestERC20 tokens to an arbitrary Namada address.
async fn test_abigen_transfer_eth_to_nam(client: Arc<Provider<Http>>) -> eyre::Result<()> {
    let bridge_address = "0x5FC8d32690cc91D4c39d9d3abcBD16989F875707".parse::<Address>()?;
    let test_erc20_address = "0x5FbDB2315678afecb367f032d93F642f64180aa3".parse::<Address>()?;

    let bridge = bridge::Bridge::new(bridge_address, Arc::clone(&client));
    let test_erc20 = test_erc20::TestERC20::new(test_erc20_address, client);

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
