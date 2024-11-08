pub use election_status::record_election;
pub use metrics_exporter_prometheus::PrometheusHandle;

use metrics::describe_gauge;
use metrics_exporter_prometheus::PrometheusBuilder;

pub fn setup_metrics_recorder() -> anyhow::Result<PrometheusHandle> {
    let handle = PrometheusBuilder::new().install_recorder()?;
    describe_gauge!(election_status::TARGET, election_status::DESCRIPTION);
    metrics::gauge!(election_status::TARGET)
        .set(election_status::ElectionStatus::Unitialized as u32);
    Ok(handle)
}

pub(super) mod election_status {
    use crate::types::ElectionResult;

    pub(super) const TARGET: &str = "polkadot_election_status";
    pub(super) const DESCRIPTION: &str = "The outcome of the most recent election represented as an integer. 0 if no election has occurred yet this is a placeholder value, 1 if the election succeeded based on an unsigned solution, 2 if the election succeeded based on a signed solution or 3 if the election failed.";
    pub(super) enum ElectionStatus {
        Unitialized = 0,
        Unsigned = 1,
        Signed = 2,
        Failed = 3,
    }

    pub fn record_election(election_result: &ElectionResult) {
        let val = match election_result {
            ElectionResult::Failed => ElectionStatus::Failed,
            ElectionResult::Unsigned => ElectionStatus::Unsigned,
            ElectionResult::Signed(_) => ElectionStatus::Signed,
        };
        metrics::gauge!(TARGET).set(val as u32);
    }
}
