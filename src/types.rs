// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

pub type RpcClient = subxt::backend::legacy::LegacyRpcMethods<subxt::PolkadotConfig>;
pub type ChainClient = subxt::OnlineClient<subxt::PolkadotConfig>;
pub type Hash = subxt::utils::H256;
pub type Header = subxt::config::substrate::SubstrateHeader<
    u32,
    <subxt::PolkadotConfig as subxt::Config>::Hasher,
>;
pub type BlockRef = subxt::blocks::BlockRef<Hash>;
pub type ExtrinsicDetails = subxt::blocks::ExtrinsicDetails<subxt::PolkadotConfig, ChainClient>;

pub use subxt::config::Header as HeaderT;

use oasgen::OaSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;
use subxt::{backend::rpc::reconnecting_rpc_client::ExponentialBackoff, utils::H256};
use url::Url;

/// Represent the result of reading a block.
pub enum ReadBlock {
    /// Election completed and the winner is known.
    ElectionFinalized(sp_npos_elections::ElectionScore),
    /// Phase closed, no more submissions expected.
    PhaseClosed,
    /// No more blocks to read.
    Done,
}

#[derive(Debug)]
struct ActiveRound {
    round: u32,
    start_block: u64,
    last_block: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElectionResult {
    // Signed submission was granted as winner
    Signed(Address),
    // Election failed i.e, no winner was selected
    Failed,
    // No signed solution was submitted and the election was finalized offchain.
    //
    // There is no event for this and if the election is finalized without a reward
    // then the election was finalized by offchain solution.
    Unsigned,
}

impl Default for ElectionResult {
    fn default() -> Self {
        Self::Unsigned
    }
}

impl std::fmt::Display for ElectionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Signed(_) => f.write_str("signed"),
            Self::Failed => f.write_str("failed"),
            Self::Unsigned => f.write_str("unsigned"),
        }
    }
}

/// Represents the state of an election round which needs be reset after the election is finalized.
#[derive(Debug)]
pub struct ElectionRound {
    result: ElectionResult,
    inner: Option<ActiveRound>,
}

impl ElectionRound {
    pub fn new() -> Self {
        Self {
            result: ElectionResult::Unsigned,
            inner: None,
        }
    }

    pub fn waiting_for_election_finalized(&self) -> bool {
        self.inner.is_some()
    }

    pub fn new_block(&mut self, block: u64, round: u32) {
        let state = match self.inner.as_mut() {
            Some(state) => state,
            None => {
                self.inner = Some(ActiveRound {
                    round,
                    start_block: block,
                    last_block: block,
                });
                return;
            }
        };

        // ElectionFinalized is emitted in the next round
        // so we need to wait for it to be emitted before
        // clearing the state and starting a new round.
        //
        // However
        if round == state.round {
            state.last_block = block;
        }
    }

    pub fn first_block(&self) -> Option<u64> {
        self.inner.as_ref().map(|s| s.start_block)
    }

    pub fn clear(&mut self) {
        self.result = ElectionResult::default();
        self.inner = None;
    }

    pub fn set_winner(&mut self, winner: Address) {
        assert!(matches!(self.result, ElectionResult::Unsigned));
        self.result = ElectionResult::Signed(winner);
    }

    pub fn election_failed(&mut self) {
        assert!(matches!(self.result, ElectionResult::Unsigned));
        self.result = ElectionResult::Failed;
    }

    pub fn complete(&mut self) -> (ElectionResult, u32) {
        let state = self
            .inner
            .take()
            .expect("At least one block must be processed in the ElectionRound; qed");
        (std::mem::take(&mut self.result), state.round)
    }
}

/// Connects to a Substrate node and provides access to chain APIs.
#[derive(Clone, Debug)]
pub struct Client {
    /// Access to typed rpc calls from subxt.
    rpc: RpcClient,
    /// Access to chain APIs such as storage, events etc.
    chain_api: ChainClient,
    /// The chain being used.
    chain_name: String,
}

impl Client {
    pub async fn new(url: Url) -> anyhow::Result<Self> {
        let rpc = {
            let rpc = subxt::backend::rpc::reconnecting_rpc_client::RpcClient::builder()
                .max_request_size(u32::MAX)
                .max_response_size(u32::MAX)
                .retry_policy(
                    ExponentialBackoff::from_millis(100)
                        .max_delay(std::time::Duration::from_secs(10)),
                )
                .request_timeout(std::time::Duration::from_secs(600))
                .build(url.as_str())
                .await?;
            subxt::backend::rpc::RpcClient::new(rpc)
        };

        let chain_api = ChainClient::from_rpc_client(rpc.clone()).await?;
        let rpc = RpcClient::new(rpc);

        let runtime_version = rpc.state_get_runtime_version(None).await?;
        let chain_name = match runtime_version.other.get("specName") {
            Some(serde_json::Value::String(n)) => n.clone(),
            Some(_) => return Err(anyhow::anyhow!("specName is not a string")),
            None => return Err(anyhow::anyhow!("specName not found")),
        };

        Ok(Self {
            rpc,
            chain_api,
            chain_name,
        })
    }

    /// Get a reference to the RPC interface exposed by subxt.
    pub fn rpc(&self) -> &RpcClient {
        &self.rpc
    }

    /// Get a reference to the chain API.
    pub fn chain_api(&self) -> &ChainClient {
        &self.chain_api
    }

    /// Get the chain name.
    pub fn chain_name(&self) -> &str {
        self.chain_name.as_str()
    }
}

#[derive(Clone, Debug, PartialEq, OaSchema)]
pub struct Address(String);

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Address {
    pub fn unsigned() -> Self {
        Self("unsigned".to_string())
    }

    pub fn signed(addr: Hash) -> Self {
        Self(format!("{:?}", addr))
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::signed(Hash::from_slice(bytes))
    }
}

impl FromStr for Address {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        Ok(match s.to_lowercase().trim() {
            "unsigned" => Self::unsigned(),
            other => H256::from_str(other)
                .map(Self::signed)
                .map_err(|e| format!("{e}"))?,
        })
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Address(s))
    }
}

impl Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}
