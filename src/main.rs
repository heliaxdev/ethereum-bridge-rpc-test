use std::convert::TryFrom;
use std::fs;
use std::sync::Arc;

use ethers::abi::FixedBytes;
use ethers::contract::Contract;
use ethers::core::{abi::Abi, types::Address};
use ethers::providers::{Http, Provider};
use eyre::WrapErr;

ethers::contract::abigen!(
    Bridge,
    "res/Bridge.abi";

    TestERC20,
    "res/TestERC20.abi";
);

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Bridge.sol contract
    let bridge_address = "0x5FC8d32690cc91D4c39d9d3abcBD16989F875707".parse::<Address>()?;
    let bridge_abi: Abi = serde_json::from_str(&{
        let path = "res/Bridge.abi";
        fs::read_to_string(path).wrap_err_with(|| format!("Failed to read file path: {path}"))?
    })
    .wrap_err("Failed to parse json")?;

    // connect to the network
    let client =
        Provider::<Http>::try_from("http://localhost:8545").wrap_err("Failed to get provider")?;

    test_bridge(&client).await?;
    test_bridge2(&client).await?;

    // create the contract object at the address
    let contract = Contract::new(bridge_address, bridge_abi, client);

    // Calling constant methods is done by calling `call()` on the method builder.
    // (if the function takes no arguments, then you must use `()` as the argument)
    let init_value = contract
        .method::<_, FixedBytes>("currentValidatorSetHash", ())?
        .call()
        .await
        .wrap_err("Failed to get currentValidatorSetHash")?;

    println!("{init_value:?}");

    // Non-constant methods are executed via the `send()` call on the method builder.
    //let call = contract.method::<_, H256>("setValue", "hi".to_owned())?;
    //let pending_tx = call.send().await?;

    // `await`ing on the pending transaction resolves to a transaction receipt
    //let receipt = pending_tx.confirmations(6).await?;

    Ok(())
}

async fn test_bridge(client: &Provider<Http>) -> eyre::Result<()> {
    let addr = "0x5FC8d32690cc91D4c39d9d3abcBD16989F875707".parse::<Address>()?;
    let bridge = Bridge::new(addr, Arc::new(client.clone()));
    let wtf = bridge.current_validator_set_hash().call().await?;
    println!("{wtf:?}");
    Ok(())
}

async fn test_bridge2(client: &Provider<Http>) -> eyre::Result<()> {
    let bridge_address = "0x5FC8d32690cc91D4c39d9d3abcBD16989F875707".parse::<Address>()?;
    let test_erc20_address = "0x5FbDB2315678afecb367f032d93F642f64180aa3".parse::<Address>()?;

    let client = Arc::new(client.clone());

    let bridge = Bridge::new(bridge_address.clone(), Arc::clone(&client));
    let test_erc20 = TestERC20::new(test_erc20_address.clone(), client);

    let approve_result = test_erc20.approve(bridge_address, 100.into()).await?;

    if !approve_result {
        panic!("Tx not approved");
    }

    let transfers = vec![NamadaTransfer {
        from: test_erc20_address,
        to: "atest1v4ehgw36xuunwd6989prwdfkxqmnvsfjxs6nvv6xxucrs3f3xcmns3fcxdzrvvz9xverzvzr56le8f"
            .into(),
        amount: 100.into(),
    }];

    let transf_result_call = bridge.transfer_to_namada(transfers, 1.into());
    let pending_tx = transf_result_call.send().await?;

    let transf_result = pending_tx.confirmations(3).await?;
    println!("{transf_result:?}");

    Ok(())
}
