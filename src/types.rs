// Copyright 2024 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

#[subxt::subxt(
    runtime_metadata_path = "artifacts/metadata.scale",
    derive_for_all_types = "Clone, Debug, Eq, PartialEq",
    substitute_type(
        path = "sp_npos_elections::ElectionScore",
        with = "::subxt::utils::Static<::sp_npos_elections::ElectionScore>"
    ),
    substitute_type(
        path = "pallet_election_provider_multi_phase::Phase",
        with = "::subxt::utils::Static<pallet_election_provider_multi_phase::Phase<u32>>"
    )
)]
pub mod runtime {}

pub type RpcClient = subxt::backend::legacy::LegacyRpcMethods<subxt::PolkadotConfig>;
pub type ChainClient = subxt::OnlineClient<subxt::PolkadotConfig>;
pub type Hash = subxt::ext::sp_core::H256;
pub type Header = subxt::config::substrate::SubstrateHeader<
    u32,
    <subxt::PolkadotConfig as subxt::Config>::Hasher,
>;

pub type EpmPhase = subxt::utils::Static<pallet_election_provider_multi_phase::Phase<u32>>;
pub use subxt::config::Header as HeaderT;
pub type ExtrinsicDetails = subxt::blocks::ExtrinsicDetails<subxt::PolkadotConfig, ChainClient>;

use oasgen::OaSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;
use subxt::{backend::rpc::reconnecting_rpc_client::ExponentialBackoff, utils::H256};
use url::Url;

pub const EPM_PALLET_NAME: &str = "ElectionProviderMultiPhase";

#[derive(Debug)]
struct ActiveRound {
    round: u32,
    start_block: u64,
    last_block: u64,
}

/// Represents the state of an election round which needs be reset after the election is finalized.
#[derive(Debug)]
pub struct ElectionRound {
    winner: Option<Address>,
    inner: Option<ActiveRound>,
}

impl ElectionRound {
    pub fn new() -> Self {
        Self {
            winner: None,
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
        self.winner = None;
        self.inner = None;
    }

    pub fn set_winner(&mut self, winner: Address) {
        assert!(self.winner.is_none());
        self.winner = Some(winner);
    }

    pub fn complete(&mut self) -> Option<Address> {
        self.inner.take();
        self.winner.take()
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
            let rpc = subxt::backend::rpc::reconnecting_rpc_client::Client::builder()
                .max_request_size(u32::MAX)
                .max_response_size(u32::MAX)
                .retry_policy(
                    ExponentialBackoff::from_millis(100)
                        .max_delay(std::time::Duration::from_secs(10)),
                )
                .request_timeout(std::time::Duration::from_secs(600))
                .build(url.as_str().into())
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

/// The chain being used.
#[derive(Debug, Copy, Clone)]
pub enum Chain {
    Westend,
    Kusama,
    Polkadot,
}

impl Chain {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Polkadot => "polkadot",
            Self::Kusama => "kusama",
            Self::Westend => "westend",
        }
    }
}

impl std::fmt::Display for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Chain {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "polkadot" => Ok(Self::Polkadot),
            "kusama" => Ok(Self::Kusama),
            "westend" => Ok(Self::Westend),
            chain => Err(format!("Invalid chain: {}", chain)),
        }
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
