mod contracts;
mod namada_queries;
mod types;

use std::convert::TryFrom;

use std::sync::Arc;

use ethbridge_governance_contract::Governance;
use ethbridge_structs::{Signature, ValidatorSetArgs};
use ethers::core::types::Address;
use ethers::providers::{Http, Provider};
use ethers::types::{H160, U256};
use eyre::WrapErr;
use crate::namada_queries::LEDGER_ADDRESS;

use self::namada_queries::{ExecuteQuery, QueryExecutor};

/// Arguments to the validator set update relay call.
struct RelayArgs {
    /// The set of active validators in the bridge.
    bridge_validators: Vec<H160>,
    /// The voting powers of the set of active validators in the bridge.
    bridge_voting_powers: Vec<U256>,
    /// The epoch of the set of active validators in the bridge.
    bridge_current_epoch: U256,
    /// The epoch of the next set of validators in the bridge.
    next_active_epoch: U256,
    /// A hash of the next set of validators' hot keys.
    next_bridge_validator_set_hash: [u8; 32],
    /// A hash of the next set of validators' cold keys.
    next_governance_validator_set_hash: [u8; 32],
    /// The signatures over the next set of validators.
    proof: Vec<Signature>,
}

/// Arguments to the validator set update relay call.
struct BPProofArgs {
    /// The set of active validators.
    validators: Vec<H160>,
    /// The voting powers of the set of active validators.
    voting_powers: Vec<U256>,
    /// Epoch
    epoch: U256,
    /// Some signatures
    signatures: Vec<Signature>,
    /// Some transfer to Ethereum
    transfers: Vec<Erc20Transfer>,
    /// The root of the bridge pool
    pool_root: [u8; 32],
    /// A merkle proof
    proof: Vec<[u8; 32]>,
    /// Augmented data for the merkle proof
    proof_flags: Vec<bool>,
    /// replay protection
    nonce: U256,
    /// The Namada rewards address for relaying
    relayer_address: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    match std::env::args()
        .nth(1)
        .unwrap()
        .as_str() {
        "valset" => {
            let Some(epoch) = std::env::args()
                .nth(2)
                .map(|x| x.parse::<u64>())
                .transpose()
                .wrap_err("Failed to parse epoch")? else {
                eyre::bail!("No epoch argument provided");
            };
            relay_test(epoch).await
        },
        "bridge-pool" => {
            bridge_pool_test().await
        },
        wrong @ _ => panic!(
            "Unsupported first argument: {}.\n\
             Expected 'valset' or 'bridge-pool'",
            wrong
        )
    }

}

async fn relay_test(epoch: u64) -> eyre::Result<()> {
    if epoch == 0 {
        eyre::bail!("Epoch value must be greater than 0");
    }

    let bridge_current_epoch = epoch.saturating_sub(2);
    println!("Fetching active validator set of epoch {bridge_current_epoch}...");

    let query = QueryExecutor::active_validator_set().at_epoch(bridge_current_epoch);
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
        bridge_validators: valset_args.validators,
        bridge_voting_powers: valset_args.powers,
        bridge_current_epoch: valset_args.nonce,
        next_active_epoch: epoch.into(),
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
        bridge_validators,
        bridge_voting_powers,
        bridge_current_epoch,
        next_active_epoch,
        next_bridge_validator_set_hash,
        next_governance_validator_set_hash,
        proof,
    } = args;

    let governance_address = "0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9".parse::<Address>()?;
    let governance = Governance::new(governance_address, client);

    let relay_op = governance
        .update_validators_set(
            ValidatorSetArgs {
                validators: bridge_validators,
                powers: bridge_voting_powers,
                nonce: bridge_current_epoch,
            },
            next_bridge_validator_set_hash,
            next_governance_validator_set_hash,
            proof,
            next_active_epoch,
        )
        .gas(600_000);
    let pending_tx = relay_op.send().await?;

    // The method `.send()` is used for mutable calls. For this reason, it
    // needs a certain number of block confirmations.
    let transf_result = pending_tx.confirmations(1).await?;
    println!("{transf_result:?}");

    Ok(())
}

/// Test that relaying a bridge pool proof works.
async fn bridge_pool_test() -> eyre::Result<()> {
    let transfers = QueryExecutor::signed_bridge_pool_transfers()
        .ledger_address(std::env::var("LEDGER").unwrap_or(LEDGER_ADDRESS.into()))
        .base_dir(std::env::var("BASE_DIR").unwrap_or(String::from(".namada").into()))
        .execute_query()?;
    let relayer = transfers
        .bridge_pool_contents
        .iter()
        .next()
        .unwrap()
        .1
        .transfer
        .sender
        .clone();
    let proof = QueryExecutor::bridge_pool_proof()
        .ledger_address(std::env::var("LEDGER").unwrap_or(LEDGER_ADDRESS.into()))
        .base_dir(std::env::var("BASE_DIR").unwrap_or(String::from(".namada").into()))
        .hashes(transfers.bridge_pool_contents.keys().cloned().collect())
        .with_relayer(relayer)
        .execute_query()?
        .proof;
    println!("Got proof:\n {:?}", proof);
    Ok(())
}

/// Relay a bridge pool proof to Ethereum.
async fn relay_bp_proof(client: Arc<Provider<Http>>, args: BPProofArgs) -> eyre::Result<()> {
    let BPProofArgs {
        validators,
        voting_powers,
        epoch,
        signatures,
        transfers,
        pool_root,
        proof,
        proof_flags,
        nonce,
        relayer_address,
    } = args;

    let bridge_address = "0xcf7ed3acca5a467e9e704c703e8d87f634fb0fc9".parse::<Address>()?;
    let bridge = Bridge::new(bridge_address, client);
    let relay_op = bridge.transfer_to_erc(
        bridge::ValidatorSetArgs {
            validators,
            powers: voting_powers,
            nonce: epoch,
        },
        signatures,
        transfers,
        pool_root,
        proof,
        proof_flags,
        nonce,
        relayer_address,
    )
    .gas(600_000);
    let pending_tx = relay_op.send().await?;

    // The method `.send()` is used for mutable calls. For this reason, it
    // needs a certain number of block confirmations.
    let transf_result = pending_tx.confirmations(1).await?;
    println!("{transf_result:?}");

    Ok(())
}
