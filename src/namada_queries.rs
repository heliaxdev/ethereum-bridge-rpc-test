//! Queries to Namada.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::process::Command;

use ethers::abi::{self, ParamType, Tokenizable};
use eyre::{eyre, WrapErr};
use serde::{Serialize, Deserialize};
use crate::contracts::governance::{Signature, ValidatorSetArgs};
use crate::types::PendingTransfer;

/// Query to the active set of validators
/// at some epoch.
pub struct ActiveValidatorSet {epoch: Option<u64>}

/// Query to a proof of the active set
/// of validators at some epoch.
pub struct ValidatorSetUpdateProof {epoch: Option<u64>}

/// Query to et the transfers in the bridge pool
/// covered with a signed Merkle root.
pub struct ProvableBridgePoolContents {}

/// A json serializable representation of the Ethereum
/// bridge pool.
#[derive(Serialize, Deserialize)]
pub struct BridgePoolResponse {
    bridge_pool_contents: HashMap<String, PendingTransfer>,
}

/// Query to construct a Merkle proof of a given set of
/// transfers in the bridge pool.
pub struct BridgePoolProof {hashes: Vec<String>, relayer: String}

/// Execute queries to ABI encoded data in Namada.
pub struct QueryExecutor<K> {
    query: K,
    global_args: Vec<(String, String)>,
}

impl<K> QueryExecutor<K> {
    pub fn arg(&mut self, key: String, value: String) {
        self.global_args.push((key, value));
    }
}

impl<K> Deref for QueryExecutor<K> {
    type Target = K;

    fn deref(&self) -> &Self::Target {
        &self.query
    }
}

impl<K> DerefMut for QueryExecutor<K> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.query
    }
}

impl QueryExecutor<ActiveValidatorSet> {
    /// Query the active set of validators.
    pub fn active_validator_set() -> Self {
        Self {
            query: ActiveValidatorSet{epoch: None},
            global_args: Default::default(),
        }
    }

    pub fn at_epoch(mut self, epoch: u64) -> Self {
        self.epoch = Some(epoch);
        self
    }
}

impl QueryExecutor<ValidatorSetUpdateProof> {
    /// Query a validator set update proof.
    pub fn validator_set_update_proof() -> Self {
        Self {
            query: ValidatorSetUpdateProof{epoch: None},
            global_args: Default::default(),
        }
    }

    pub fn at_epoch(mut self, epoch: u64) -> Self {
        self.epoch = Some(epoch);
        self
    }
}

impl QueryExecutor<ProvableBridgePoolContents> {
    pub fn signed_bridge_pool_transfers() -> Self {
        Self {
            query: ProvableBridgePoolContents{},
            global_args: Default::default(),
        }
    }
}

impl QueryExecutor<BridgePoolProof> {
    pub fn signed_bridge_pool_transfers() -> Self {
        Self {
            query: BridgePoolProof{hashes: Default::default(), relayer: "".into()},
            global_args: Default::default(),
        }
    }

    pub fn hashes(&mut self, hashes: Vec<String>) {
        self.hashes = hashes;
    }

    pub fn with_relayer(&mut self, relayer: String) {
        self.relayer = relayer;
    }
}

/// Execute a query, and return its response.
pub trait ExecuteQuery {
    /// Response data.
    type Response;

    /// Execute a query, and return its response.
    fn execute_query(&self) -> eyre::Result<Self::Response>;
}

impl ExecuteQuery for QueryExecutor<ActiveValidatorSet> {
    type Response = ValidatorSetArgs;

    fn execute_query(&self) -> eyre::Result<Self::Response> {
        let abi_data = query_valset_abi_data("active", self.epoch, &self.global_args)?;

        let params = [ParamType::Tuple(vec![
            ParamType::Array(Box::new(ParamType::Address)),
            ParamType::Array(Box::new(ParamType::Uint(256))),
            ParamType::Uint(256),
        ])];
        let Ok(Some(token)) = abi::decode(&params, &abi_data).map(|mut t| t.pop()) else {
            eyre::bail!("Invalid active valset ABI encoded data");
        };
        Ok(Tokenizable::from_token(token)
            .expect("Decoding shouldn't fail, given we type checked already"))
    }
}

impl ExecuteQuery for QueryExecutor<ValidatorSetUpdateProof> {
    type Response = ([u8; 32], [u8; 32], Vec<Signature>);

    fn execute_query(&self) -> eyre::Result<Self::Response> {
        let abi_data = query_valset_abi_data("proof", self.epoch, &self.global_args)?;

        let params = [ParamType::Tuple(vec![
            ParamType::FixedBytes(32),
            ParamType::FixedBytes(32),
            ParamType::Array(Box::new(ParamType::Tuple(vec![
                ParamType::FixedBytes(32),
                ParamType::FixedBytes(32),
                ParamType::Uint(8),
            ]))),
        ])];
        let Ok(Some(token)) = abi::decode(&params, &abi_data).map(|mut t| t.pop()) else {
            eyre::bail!("Invalid valset proof ABI encoded data");
        };
        Ok(Tokenizable::from_token(token)
            .expect("Decoding shouldn't fail, given we type checked already"))
    }
}

impl ExecuteQuery for QueryExecutor<ProvableBridgePoolContents> {
    type Response = BridgePoolResponse;

    fn execute_query(&self) -> eyre::Result<Self::Response> {
        let bp_data = query_bridge_pool_data("query-signed", &self.global_args)?;
        serde_json::from_str(&bp_data)
            .map_err(|e| eyre!("Could not parse response as json due to {:?}.", e))
    }
}

impl ExecuteQuery for QueryExecutor<BridgePoolProof> {
    type Response = BridgePoolResponse;

    fn execute_query(&self) -> eyre::Result<Self::Response> {
        let mut extra_args = self.global_args.clone();
        extra_args.push(("--relayer".into(), self.relayer.clone()));
        let mut hashes_str = String::from("");
        for hash in &self.hashes {
            hashes_str.push_str(&format!("{} ", hash));
        }
        let hashes_str = format!(r#""{}""#, hashes_str);
        extra_args.push(("--hashes".into(), hashes_str));
        let bp_data = query_bridge_pool_data(
            "query-signed",
            &extra_args
        )?;
        serde_json::from_str(&bp_data)
            .map_err(|e| eyre!("Could not parse response as json due to {:?}.", e))
    }
}

/// Fetch ABI encoded data for the given validator set relayer command.
fn query_valset_abi_data(command: &str, epoch: Option<u64>, global_args: &[(String, String)]) -> eyre::Result<Vec<u8>> {
    let mut cmd = Command::new("namadar");

    cmd.arg("validator-set").arg(command);
    if let Some(epoch) = epoch {
        cmd.arg("--epoch").arg(format!("{epoch}"));
    }
    for (key, value) in global_args {
        cmd.arg(key).arg(value);
    }

    let output = cmd.output().wrap_err("Failed to execute `namadar`")?;
    if output.status.code() != Some(0) {
        eyre::bail!("The Namada relayer halted unexpectedly; is the ledger running?");
    }

    // fetch hex data as str and validate it
    let abi_data = std::str::from_utf8(&output.stdout)
        .wrap_err("Invalid UTF-8 data in the relayer's response")?
        .trim();

    let prefix = abi_data
        .get(..2)
        .ok_or_else(|| eyre::eyre!("Short relayer response"))?;
    if prefix != "0x" {
        eyre::bail!("The Namada relayer response did not return ABI encoded data in string form");
    }

    // decode hex data
    hex::decode(&abi_data[2..]).wrap_err("Invalid hex encoded ABI data")
}


/// Fetch ABI encoded data from the given bridge pool command.
fn query_bridge_pool_data(command: &str, extra_args: &[(String, String)]) -> eyre::Result<String> {
    let mut cmd = Command::new("namadar");

    cmd.arg("ethereum-bridge-pool")
        .arg(command);
    for (key, value) in extra_args {
        cmd.arg(key).arg(value);
    }
    let output = cmd.output().wrap_err("Failed to execute `namadar`")?;
    if output.status.code() != Some(0) {
        eyre::bail!("The Namada relayer halted unexpectedly; is the ledger running?");
    }

    // fetch hex data as str and validate it
    Ok(
        std::str::from_utf8(&output.stdout)
            .wrap_err("Invalid UTF-8 data in the relayer's response")?
            .trim()
           .into()
    )
}
