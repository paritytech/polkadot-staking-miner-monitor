pub use metrics_exporter_prometheus::PrometheusHandle;

use crate::types::ElectionResult;
use metrics::describe_gauge;
use metrics_exporter_prometheus::PrometheusBuilder;

const TARGET: &str = "polkadot_election_status";
const DESCRIPTION: &str = "The outcome of the most recent election represented as a integer. 0 if the election failed, 1 if the election succeeded based on unsigned solution, 2 if the election succeeded based on signed solution or 3 if no election has occurred yet";

const FAILED: u32 = 0;
const UNSIGNED: u32 = 1;
const SIGNED: u32 = 2;
const WAITING_FOR_ELECTION: u32 = 3;

pub fn setup_metrics_recorder() -> anyhow::Result<PrometheusHandle> {
    let handle = PrometheusBuilder::new().install_recorder()?;
    describe_gauge!(TARGET, DESCRIPTION);
    metrics::gauge!(TARGET, "handle" => "handle").set(WAITING_FOR_ELECTION);
    Ok(handle)
}

pub fn record_election(election_result: &ElectionResult) {
    let val = match election_result {
        ElectionResult::Failed => FAILED,
        ElectionResult::Unsigned => UNSIGNED,
        ElectionResult::Signed(_) => SIGNED,
    };
    metrics::gauge!(TARGET, "handle" => "handle").set(val);
}
