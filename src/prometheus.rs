use futures::channel::oneshot;
pub use hidden::*;
use hyper::{
    header::CONTENT_TYPE,
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response,
};
use prometheus::{Encoder, TextEncoder};

async fn serve_req(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let response = match (req.method(), req.uri().path()) {
        (&Method::GET, "/metrics") => {
            let mut buffer = vec![];
            let encoder = TextEncoder::new();
            let metric_families = prometheus::gather();
            encoder.encode(&metric_families, &mut buffer).unwrap();

            Response::builder()
                .status(200)
                .header(CONTENT_TYPE, encoder.format_type())
                .body(Body::from(buffer))
                .unwrap()
        }
        (&Method::GET, "/") => Response::builder()
            .status(200)
            .body(Body::from(""))
            .unwrap(),
        _ => Response::builder()
            .status(404)
            .body(Body::from(""))
            .unwrap(),
    };

    Ok(response)
}

pub struct GracefulShutdown(Option<oneshot::Sender<()>>);

impl Drop for GracefulShutdown {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            let _ = handle.send(());
        }
    }
}

pub fn run(port: u16) -> Result<GracefulShutdown, String> {
    let (tx, rx) = oneshot::channel();

    // For every connection, we must make a `Service` to handle all incoming HTTP requests on said
    // connection.
    let make_svc = make_service_fn(move |_conn| async move {
        Ok::<_, std::convert::Infallible>(service_fn(serve_req))
    });

    let addr = ([0, 0, 0, 0], port).into();
    let server = hyper::Server::try_bind(&addr)
        .map_err(|e| format!("Failed bind socket on port {} {:?}", port, e))?
        .serve(make_svc);

    tracing::info!("Started prometheus endpoint on http://{}", addr);

    let graceful = server.with_graceful_shutdown(async {
        rx.await.ok();
    });

    tokio::spawn(async move {
        if let Err(e) = graceful.await {
            tracing::warn!("Server error: {}", e);
        }
    });

    Ok(GracefulShutdown(Some(tx)))
}

mod hidden {
    use once_cell::sync::Lazy;
    use prometheus::{register_counter_vec, CounterVec};
    use sp_npos_elections::ElectionScore;
    use subxt::ext::sp_core::H256 as Hash;

    static SUBMISSIONS_PER_ROUND: Lazy<CounterVec> = Lazy::new(|| {
        register_counter_vec!(
            "epm_submissions",
            "EPM submissions per round",
            &["chain", "round", "address", "score"]
        )
        .unwrap()
    });

    static ELECTION_WINNER: Lazy<CounterVec> = Lazy::new(|| {
        register_counter_vec!(
            "epm_election_winner",
            "EPM election winner per round",
            &["chain", "round", "address", "score"]
        )
        .unwrap()
    });

    pub fn submission(chain: &str, round: u32, addr: Option<Hash>, score: ElectionScore) {
        let round = round.to_string();
        let addr = serialize_addr(addr);
        let score = serialize_score(score);

        SUBMISSIONS_PER_ROUND
            .with_label_values(&[chain, &round, &addr, &score])
            .inc();
    }

    pub fn election_winner(chain: &str, round: u32, addr: Option<Hash>, score: ElectionScore) {
        let round = round.to_string();
        let addr = serialize_addr(addr);
        let score = serialize_score(score);

        ELECTION_WINNER
            .with_label_values(&[chain, &round, &addr, &score])
            .inc();
    }

    fn serialize_score(score: ElectionScore) -> String {
        format!(
            "{}",
            serde_json::json!({
                "score_minimal_stake": score.minimal_stake.to_string(),
                "score_sum_squared": score.sum_stake_squared.to_string(),
                "score_sum_stake": score.sum_stake.to_string()
            })
        )
    }

    fn serialize_addr(addr: Option<Hash>) -> String {
        addr.map_or_else(|| "unsigned".to_string(), |a| format!("{:?}", a))
    }
}
