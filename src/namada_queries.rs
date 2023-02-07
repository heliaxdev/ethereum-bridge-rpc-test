//! Queries to Namada.

use std::marker::PhantomData;
use std::process::Command;

use ethers::abi::{self, ParamType, Tokenizable};
use eyre::WrapErr;

use crate::contracts::governance::{Signature, ValidatorSetArgs};

/// Tag type to indicate a query to the active set of validators
/// at some epoch.
pub enum ActiveValidatorSet {}

/// Tag type to indicate a query to a proof of the active set
/// of validators at some epoch.
pub enum ValidatorSetUpdateProof {}

/// Tag type to indicate a query to et the transfers in the
/// bridge pool covered with a signed Merkle root.
pub enum ProvableBridgePoolContents {}

/// Tag type to indicate a query to construct a Merkle
/// proof of a given set of transfers in the bridge pool.
pub enum BridgePoolProof {}

/// Execute queries to ABI encoded data in Namada.
pub struct QueryExecutor<K> {
    epoch: Option<u64>,
    _kind: PhantomData<*const K>,
}

impl<K> QueryExecutor<K> {
    /// Configure the epoch to query.
    pub fn at_epoch(mut self, epoch: u64) -> Self {
        self.epoch = Some(epoch);
        self
    }
}

impl QueryExecutor<ActiveValidatorSet> {
    /// Query the active set of validators.
    pub fn active_validator_set() -> Self {
        Self {
            _kind: PhantomData,
            epoch: None,
        }
    }
}

impl QueryExecutor<ValidatorSetUpdateProof> {
    /// Query a validator set update proof.
    pub fn validator_set_update_proof() -> Self {
        Self {
            _kind: PhantomData,
            epoch: None,
        }
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
        let abi_data = query_valset_abi_data("active", self.epoch)?;

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
        let abi_data = query_valset_abi_data("proof", self.epoch)?;

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

/// Fetch ABI encoded data for the given validator set relayer command.
fn query_valset_abi_data(command: &str, epoch: Option<u64>) -> eyre::Result<Vec<u8>> {
    let mut cmd = Command::new("namadar");

    cmd.arg("validator-set").arg(command);
    if let Some(epoch) = epoch {
        cmd.arg("--epoch").arg(format!("{epoch}"));
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
fn query_bridge_pool_data(command: &str, epoch: Option<u64>) -> eyre::Result<Vec<u8>> {
    let mut cmd = Command::new("namadar");

    cmd.arg("ethereum-bridge-pool")
        .arg(command);

    let output = cmd.output().wrap_err("Failed to execute `namadar`")?;
    if output.status.code() != Some(0) {
        eyre::bail!("The Namada relayer halted unexpectedly; is the ledger running?");
    }

    // fetch hex data as str and validate it
    let data = std::str::from_utf8(&output.stdout)
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
