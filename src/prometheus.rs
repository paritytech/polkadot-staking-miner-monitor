pub use metrics_exporter_prometheus::PrometheusHandle;

use crate::types::ElectionResult;
use metrics::describe_gauge;
use metrics_exporter_prometheus::PrometheusBuilder;

const TARGET: &str = "polkadot_election_status";
const DESCRIPTION: &str = "The outcome of the most recent election represented as a integer. 0 if the election failed, 1 if the election succeeded based on unsigned solution and 2 if the election succeeded based on signed solution.";

pub fn setup_metrics_recorder() -> anyhow::Result<PrometheusHandle> {
    let handle = PrometheusBuilder::new().install_recorder()?;
    describe_gauge!(TARGET, DESCRIPTION);
    Ok(handle)
}

pub fn record_election(election_result: &ElectionResult) {
    let val = match election_result {
        ElectionResult::Failed => 0,
        ElectionResult::Unsigned => 1,
        ElectionResult::Signed(_) => 2,
    };
    metrics::gauge!(TARGET, "handle" => "handle").set(val);
}
