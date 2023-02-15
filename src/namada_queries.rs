//! Queries to Namada.

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::process::Command;

use ethbridge_structs::{RelayProof, Signature, ValidatorSetArgs};
use ethers::abi::{self, AbiDecode, ParamType, Tokenizable};
use eyre::{eyre, WrapErr};
use serde::{Serialize, Deserialize};
use crate::types::PendingTransfer;

pub const LEDGER_ADDRESS: &str = "127.0.0.1:26657";
pub const BASE_DIR: &str = ".namada";

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
    pub(crate) bridge_pool_contents: HashMap<String, PendingTransfer>,
}

/// A json serializable representation of the Ethereum
/// bridge pool proof.
#[derive(Serialize, Deserialize)]
pub struct ProofResponse {
    pub(crate) proof: Vec<u8>,
}

/// Query to construct a Merkle proof of a given set of
/// transfers in the bridge pool.
pub struct BridgePoolProof {hashes: Vec<String>, relayer: String}

/// Execute queries to ABI encoded data in Namada.
pub struct QueryExecutor<K> {
    query: K,
    ledger_address: String,
    base_dir: String,
}


impl<K> QueryExecutor<K> {
    pub fn ledger_address(mut self, addr: String) -> Self {
        self.ledger_address = addr;
        self
    }

    pub fn base_dir(mut self, dir: String) -> Self {
        self.base_dir = dir;
        self
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
            ledger_address: LEDGER_ADDRESS.into(),
            base_dir: BASE_DIR.into(),
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
            ledger_address: LEDGER_ADDRESS.into(),
            base_dir: BASE_DIR.into(),
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
            ledger_address: LEDGER_ADDRESS.into(),
            base_dir: BASE_DIR.into(),
        }
    }
}

impl QueryExecutor<BridgePoolProof> {
    pub fn bridge_pool_proof() -> Self {
        Self {
            query: BridgePoolProof{hashes: Default::default(), relayer: "".into()},
            ledger_address: LEDGER_ADDRESS.into(),
            base_dir: BASE_DIR.into(),
        }
    }

    pub fn hashes(mut self, hashes: Vec<String>) -> Self {
        self.hashes = hashes;
        self
    }

    pub fn with_relayer(mut self, relayer: String) -> Self {
        self.relayer = relayer;
        self
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
        let abi_data = query_valset_abi_data(
            &self.base_dir,
            &self.ledger_address,
            "active",
            self.epoch,
        )?;

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
        let abi_data = query_valset_abi_data(
            &self.base_dir,
            &self.ledger_address,
            "proof",
            self.epoch
        )?;

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
        let bp_data = query_bridge_pool_data(
            &self.base_dir,
            &self.ledger_address,
            "query-signed",
            vec![],
        )?;
        serde_json::from_str(&bp_data)
            .map_err(|e| eyre!("Could not parse response as json due to {:?}.", e))
    }
}

impl ExecuteQuery for QueryExecutor<BridgePoolProof> {
    type Response = RelayProof;

    fn execute_query(&self) -> eyre::Result<Self::Response> {
        let mut extra_args = vec![("--relayer".into(), self.relayer.clone())];
        let mut hashes_str = String::from("");
        for hash in &self.hashes {
            hashes_str.push_str(&format!("{} ", hash));
        }
        let hashes_str = hashes_str.trim().to_string();
        extra_args.push(("--hash-list".into(), hashes_str));
        let proof = query_bridge_pool_data(
            &self.base_dir,
            &self.ledger_address,
            "construct-proof",
            &extra_args
        )?;
        let proof = proof
            .strip_prefix("Ethereum ABI-encoded proof:\n ")
            .unwrap();

        let proof_response: ProofResponse = serde_json::from_str(proof)
            .map_err(|e| eyre!("Could not parse response as json due to {:?}.", e))?;
         RelayProof::decode(proof_response.proof)
            .wrap_err("Could not deserialize RelayProof.")
    }
}

/// Fetch ABI encoded data for the given validator set relayer command.
fn query_valset_abi_data(
    base_dir: &str,
    ledger_address: &str,
    command: &str,
    epoch: Option<u64>
) -> eyre::Result<Vec<u8>> {
    let namadar_path = std::env::var("NAMADAR")
        .unwrap_or_else(|_| String::from("namadar"));
    let mut cmd = Command::new(&namadar_path);

    cmd.args(vec!["--base-dir", base_dir, "validator-set", command]);
    if let Some(epoch) = epoch {
        cmd.arg("--epoch").arg(format!("{epoch}"));
    }
    cmd.arg("--ledger-address").arg(ledger_address);

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
fn query_bridge_pool_data(
    base_dir: &str,
    ledger_address: &str,
    command: &str,
    extra: impl AsRef<[(String, String)]>
) -> eyre::Result<String> {
    let extra_args = extra.as_ref();
    let namadar_path = std::env::var("NAMADAR")
        .unwrap_or_else(|_| String::from("namadar"));
    let mut cmd = Command::new(&namadar_path);


    cmd.args(vec!["--base-dir", base_dir, "ethereum-bridge-pool", command]);
    for (key, value) in extra_args {
        cmd.arg(key).arg(value);
    }
    cmd.arg("--ledger-address").arg(ledger_address);
    let output = cmd.output().wrap_err("Failed to execute `namadar`")?;
    if output.status.code() != Some(0) {
        println!("Failed to run command: {:?} with ouput: {:?}", cmd, output);
        eyre::bail!("The Namada relayer halted unexpectedly; is the ledger running?");
    }

    Ok(
        std::str::from_utf8(&output.stdout)
            .wrap_err("Invalid UTF-8 data in the relayer's response")?
            .trim()
           .into()
    )
}
