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

pub const EPM_PALLET_NAME: &str = "ElectionProviderMultiPhase";

/// Represents the submissions in a round and should be cleared after each round.
#[derive(Debug)]
pub struct SubmissionsInRound {
    pub submissions: Vec<Submission>,
}

impl SubmissionsInRound {
    pub fn new() -> Self {
        Self {
            submissions: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.submissions.clear();
    }

    pub fn add_submission(&mut self, submission: Submission) {
        self.submissions.push(submission);
    }
}

/// Wraps the subxt interfaces to make it easy to use for the staking-miner.
#[derive(Clone, Debug)]
pub struct Client {
    /// Access to typed rpc calls from subxt.
    rpc: RpcClient,
    /// Access to chain APIs such as storage, events etc.
    chain_api: ChainClient,
}

impl Client {
    pub async fn new(uri: &str) -> Result<Self, subxt::Error> {
        tracing::debug!("attempting to connect to {:?}", uri);

        let rpc = loop {
            match jsonrpsee::ws_client::WsClientBuilder::default()
                .max_request_size(u32::MAX)
                .max_response_size(u32::MAX)
                .request_timeout(std::time::Duration::from_secs(600))
                .build(&uri)
                .await
            {
                Ok(rpc) => break subxt::backend::rpc::RpcClient::new(rpc),
                Err(e) => {
                    tracing::warn!(
                        "failed to connect to client due to {:?}, retrying soon..",
                        e,
                    );
                }
            };
            tokio::time::sleep(std::time::Duration::from_millis(2_500)).await;
        };

        let chain_api = ChainClient::from_rpc_client(rpc.clone()).await?;
        Ok(Self {
            rpc: RpcClient::new(rpc),
            chain_api,
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
}

/// The chain being used.
#[derive(Debug, Copy, Clone)]
pub enum Chain {
    Westend,
    Kusama,
    Polkadot,
}

impl std::fmt::Display for Chain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chain = match self {
            Self::Polkadot => "polkadot",
            Self::Kusama => "kusama",
            Self::Westend => "westend",
        };
        write!(f, "{}", chain)
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
