mod contracts;
mod namada_queries;

use std::convert::TryFrom;

use std::sync::Arc;

use ethers::core::types::Address;
use ethers::providers::{Http, Provider};
use ethers::types::{H160, U256};
use eyre::WrapErr;

use self::contracts::governance::{Governance, Signature, ValidatorSetArgs};
use self::namada_queries::{ExecuteQuery, QueryExecutor};

/// Arguments to the validator set update relay call.
struct RelayArgs {
    /// The set of active validators.
    validators: Vec<H160>,
    /// The voting powers of the set of active validators.
    voting_powers: Vec<U256>,
    /// The current epoch.
    current_epoch: U256,
    /// A hash of the next set of validators' hot keys.
    next_bridge_validator_set_hash: [u8; 32],
    /// A hash of the next set of validators' cold keys.
    next_governance_validator_set_hash: [u8; 32],
    /// The signatures over the next set of validators.
    proof: Vec<Signature>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let Some(epoch) = std::env::args()
        .nth(1)
        .map(|x| x.parse::<u64>())
        .transpose()
        .wrap_err("Failed to parse epoch")? else {
        eyre::bail!("No epoch argument provided");
    };
    if epoch == 0 {
        eyre::bail!("Epoch value must be greater than 0");
    }

    let prev_epoch = epoch - 1;
    println!("Fetching active validator set of epoch {prev_epoch}...");

    let query = QueryExecutor::active_validator_set().at_epoch(prev_epoch);
    let valset_args = query.execute_query()?;

    println!("Done! Now fetching a validator set update proof...");

    let query = QueryExecutor::validator_set_update_proof().at_epoch(epoch);
    let (bridge_hash, gov_hash, proof) = query.execute_query()?;

    println!("Done! Relaying proof...");

    // Connect to the network. We are assuming that a `hardhat` node is running in the
    // background, with some contracts already deployed.
    //
    // <https://github.com/sug0/ethereum-bridge/tree/tiago/test/deploy-contracts>
    let client = Arc::new(
        Provider::<Http>::try_from("http://localhost:8545").wrap_err("Failed to get provider")?,
    );
    let args = RelayArgs {
        validators: valset_args.validators,
        voting_powers: valset_args.powers,
        current_epoch: valset_args.nonce,
        next_bridge_validator_set_hash: bridge_hash,
        next_governance_validator_set_hash: gov_hash,
        proof,
    };
    relay_proof(Arc::clone(&client), args).await?;

    println!("Validator set update successfully relayed!");
    Ok(())
}

/// Relay a validator set update to Ethereum.
async fn relay_proof(client: Arc<Provider<Http>>, args: RelayArgs) -> eyre::Result<()> {
    let RelayArgs {
        validators,
        voting_powers,
        current_epoch,
        next_bridge_validator_set_hash,
        next_governance_validator_set_hash,
        proof,
    } = args;

    let governance_address = "0x0165878A594ca255338adfa4d48449f69242Eb8F".parse::<Address>()?;
    let governance = Governance::new(governance_address, client);

    let relay_op = governance.update_validators_set(
        ValidatorSetArgs {
            validators,
            powers: voting_powers,
            nonce: current_epoch,
        },
        next_bridge_validator_set_hash,
        next_governance_validator_set_hash,
        proof,
        current_epoch + 1,
    );
    let pending_tx = relay_op.send().await?;

    // The method `.send()` is used for mutable calls. For this reason, it
    // needs a certain number of block confirmations.
    let transf_result = pending_tx.confirmations(3).await?;
    println!("{transf_result:?}");

    Ok(())
}
