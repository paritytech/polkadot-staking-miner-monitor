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
pub type Submission = (sp_npos_elections::ElectionScore, Option<Hash>, u32);
pub type Header = subxt::config::substrate::SubstrateHeader<
    u32,
    <subxt::PolkadotConfig as subxt::Config>::Hasher,
>;

pub type EpmPhase = subxt::utils::Static<pallet_election_provider_multi_phase::Phase<u32>>;
pub use subxt::config::Header as HeaderT;
pub type Address = Hash;

use std::str::FromStr;
use subxt::backend::rpc::reconnecting_rpc_client::ExponentialBackoff;

pub const EPM_PALLET_NAME: &str = "ElectionProviderMultiPhase";

#[derive(Debug)]
struct ActiveRound {
    round: u32,
    start_block: u64,
    last_block: u64,
}

/// Represents the submissions in a round and should be cleared after each round.
#[derive(Debug)]
pub struct SubmissionsInRound {
    pub submissions: Vec<Submission>,
    inner: Option<ActiveRound>,
}

impl SubmissionsInRound {
    pub fn new() -> Self {
        Self {
            submissions: Vec::new(),
            inner: None,
        }
    }

    pub fn waiting_for_election_finalized(&self) -> bool {
        self.inner.is_some()
    }

    pub fn new_block(&mut self, block: u64, round: u32) {
        if self.inner.is_none() {
            self.inner = Some(ActiveRound {
                round,
                start_block: block,
                last_block: block,
            });
        }

        let state = self.inner.as_mut().unwrap();

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
        self.submissions.clear();
        self.inner = None;
    }

    pub fn add_submission(&mut self, submission: Submission) {
        self.submissions.push(submission);
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
    chain_name: Chain,
}

impl Client {
    pub async fn new(uri: &str) -> anyhow::Result<Self> {
        let rpc = {
            let rpc = subxt::backend::rpc::reconnecting_rpc_client::Client::builder()
                .max_request_size(u32::MAX)
                .max_response_size(u32::MAX)
                .retry_policy(
                    ExponentialBackoff::from_millis(100)
                        .max_delay(std::time::Duration::from_secs(10)),
                )
                .request_timeout(std::time::Duration::from_secs(600))
                .build(uri.to_string())
                .await?;
            subxt::backend::rpc::RpcClient::new(rpc)
        };

        let chain_api = ChainClient::from_rpc_client(rpc.clone()).await?;
        let rpc = RpcClient::new(rpc);

        let runtime_version = rpc.state_get_runtime_version(None).await?;
        let spec_name = match runtime_version.other.get("specName") {
            Some(serde_json::Value::String(n)) => n.clone(),
            Some(_) => return Err(anyhow::anyhow!("specName is not a string")),
            None => return Err(anyhow::anyhow!("specName not found")),
        };
        let chain_name = Chain::from_str(&spec_name).map_err(|e| anyhow::anyhow!("{e}"))?;

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
    pub fn chain_name(&self) -> Chain {
        self.chain_name
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

pub mod kusama {
    use frame_support::traits::ConstU32;

    frame_election_provider_support::generate_solution_type!(
        #[compact]
        pub struct NposSolution24::<
            VoterIndex = u32,
            TargetIndex = u16,
            Accuracy = sp_runtime::PerU16,
            MaxVoters = ConstU32::<12500>
        >(24)
    );
}

pub mod polkadot {
    use frame_support::traits::ConstU32;

    frame_election_provider_support::generate_solution_type!(
        #[compact]
        pub struct NposSolution16::<
            VoterIndex = u32,
            TargetIndex = u16,
            Accuracy = sp_runtime::PerU16,
            MaxVoters = ConstU32::<12500>
        >(16)
    );
}

pub mod westend {
    use frame_support::traits::ConstU32;

    frame_election_provider_support::generate_solution_type!(
        #[compact]
        pub struct NposSolution16::<
            VoterIndex = u32,
            TargetIndex = u16,
            Accuracy = sp_runtime::PerU16,
            MaxVoters = ConstU32::<12500>
        >(16)
    );
}
