//! Queries to ABI encoded data in Namada.

use std::marker::PhantomData;
use std::process::Command;

use ethers::abi::{self, ParamType, Tokenizable};
use ethers::types::{H160, U256};
use eyre::WrapErr;

/// Tag type to indicate a query to the active set of validators
/// at some epoch.
enum ActiveValidatorSet {}

/// Tag type to indicate a query to a proof of the active set
/// of validators at some epoch.
enum ValidatorSetUpdateProof {}

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
    type Response = (Vec<H160>, Vec<U256>, U256);

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
        let decoded: (Vec<H160>, Vec<U256>, U256) = Tokenizable::from_token(token)
            .expect("Decoding shouldn't fail, given we type checked already");

        Ok(decoded)
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
        eyre::bail!("The Namada relayer halted unexpectedly");
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
