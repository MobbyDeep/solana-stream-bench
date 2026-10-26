use crate::{
    analysis,
    config::{ConfigToml, EndpointKind, OutputSettings},
    providers::yellowstone::build_subscription,
    report,
    utils::{Comparator, TransactionData},
};
use std::time::Duration;

pub(crate) fn config() -> ConfigToml {
    toml::from_str("[config]\ntransactions=1\naccount='11111111111111111111111111111111'\ncommitment='processed'\n[[endpoint]]\nname='A'\nurl='http://localhost:10001'\nkind='yellowstone'\n[[endpoint]]\nname='B'\nurl='http://localhost:10002'\nkind='yellowstone'\n[[endpoint]]\nname='Other feed'\nurl='http://localhost:10003'\nkind='arpc'\n").unwrap()
}
pub(crate) fn observation(ms: u64) -> TransactionData {
    TransactionData {
        elapsed_since_start: Duration::from_millis(ms),
        wallclock_secs: 100.0,
        start_wallclock_secs: 90.0,
    }
}
#[test]
fn config_defaults_keep_optional_categories_and_publishing_off() {
    let c = config();
    c.validate().unwrap();
    assert!(!c.config.yellowstone_accounts_bench_enabled && !c.backend.enabled);
    assert!(c.output.txt_file_output);
    assert_eq!(c.endpoint[0].kind, EndpointKind::Yellowstone);
    let round_trip: ConfigToml = toml::from_str(&toml::to_string(&c).unwrap()).unwrap();
    round_trip.validate().unwrap();
}
#[test]
fn account_subscription_is_optional_and_never_requests_slots() {
    for accounts in [false, true] {
        let mut c = config();
        c.config.yellowstone_accounts_bench_enabled = accounts;
        let request = build_subscription(&c.config);
        assert_eq!(request.accounts.len(), usize::from(accounts));
        assert!(request.slots.is_empty());
        assert_eq!(request.transactions.len(), 1);
        if accounts {
            assert_eq!(request.accounts["accounts"].owner, vec![c.config.account]);
            assert_eq!(
                request.accounts["accounts"].nonempty_txn_signature,
                Some(true)
            );
        }
    }
}
#[test]
fn exact_toml_account_setting_enables_subscription() {
    let input = r#"
[config]
transactions = 1
account = "11111111111111111111111111111111"
commitment = "processed"
yellowstone_accounts_bench_enabled = true

[[endpoint]]
name = "A"
url = "http://localhost:10001"
kind = "yellowstone"
"#;
    let config: ConfigToml = toml::from_str(input).unwrap();
    config.validate().unwrap();
    assert!(config.config.yellowstone_accounts_bench_enabled);
    assert_eq!(build_subscription(&config.config).accounts.len(), 1);
}
#[test]
fn account_is_shared_by_transaction_and_account_filters() {
    let mut c = config();
    c.config.yellowstone_accounts_bench_enabled = true;
    c.config.account = "Vote111111111111111111111111111111111111111".into();
    c.validate().unwrap();
    assert_eq!(
        build_subscription(&c.config).accounts["accounts"].owner,
        vec![c.config.account.clone()]
    );
    assert_eq!(
        build_subscription(&c.config).transactions["account"].account_include,
        vec![c.config.account]
    );
}
#[test]
fn invalid_configurations_are_rejected() {
    let mut c = config();
    c.config.transactions = -1;
    assert!(c.validate().is_err());
    let mut c = config();
    c.endpoint[1].name = "A".into();
    assert!(c.validate().is_err());
    let mut c = config();
    c.config.account = "bad".into();
    assert!(c.validate().is_err());
    let mut c = config();
    c.output.timestamp_format = "%Q".into();
    assert!(c.validate().is_err());
    let mut c = config();
    c.output.filename_prefix = "../outside".into();
    assert!(c.validate().is_err());
}
#[test]
fn udp_uses_upstream_bare_bind_address() {
    let mut c = config();
    c.endpoint[0].kind = EndpointKind::UdpShredstream;
    c.endpoint[0].url = "0.0.0.0:20000".into();
    c.validate().unwrap();
}
#[test]
fn upstream_transaction_intersection_and_percentiles_are_preserved() {
    let comparator = Comparator::new();
    comparator.record_observation("A", "one", observation(1), 2);
    comparator.record_observation("B", "one", observation(3), 2);
    comparator.record_observation("A", "partial", observation(4), 2);
    let names = vec!["A".into(), "B".into()];
    let summary = analysis::compute_run_summary(&comparator, &names);
    assert_eq!(summary.total_signatures, 1);
    let b = summary.endpoints.iter().find(|e| e.name == "B").unwrap();
    assert_eq!(b.p50_delay_ms, Some(2.0));
    assert_eq!(b.valid_transactions, 1);
    let text = analysis::render_run_summary(&summary);
    assert!(
        text.contains("Endpoint")
            && text.contains("Valid Tx")
            && text.contains("A")
            && text.contains("B")
    );
}
#[test]
fn txt_uses_identical_summary_bytes_and_utc_start_with_collision_suffixes() {
    use chrono::TimeZone;
    let directory = tempfile::tempdir().unwrap();
    let output = OutputSettings {
        directory: directory.path().join("new").display().to_string(),
        ..Default::default()
    };
    let started = chrono::Utc
        .with_ymd_and_hms(2026, 10, 26, 3, 19, 0)
        .unwrap();
    let text = "Solana Stream Bench\nTest summary\n";
    let mut terminal = Vec::new();
    let first = report::write_report(&output, started, text, &mut terminal)
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(&first).unwrap(), terminal);
    assert_eq!(terminal, text.as_bytes());
    assert_eq!(
        first.file_name().unwrap(),
        "solana-stream-bench_2026-10-26_03-19-00.txt"
    );
    let second = report::write_report(&output, started, text, &mut Vec::new())
        .unwrap()
        .unwrap();
    assert!(second.display().to_string().ends_with("_1.txt"));
    assert_eq!(std::fs::read(first).unwrap(), text.as_bytes());
    assert!(
        std::fs::read_dir(&output.directory).unwrap().all(|e| e
            .unwrap()
            .path()
            .extension()
            .unwrap()
            == "txt")
    );
}
#[test]
fn disabling_txt_keeps_terminal_summary_without_creating_files() {
    let output = OutputSettings {
        txt_file_output: false,
        directory: "path-should-not-be-created".into(),
        ..Default::default()
    };
    let mut terminal = Vec::new();
    assert!(
        report::write_report(&output, chrono::Utc::now(), "summary\n", &mut terminal)
            .unwrap()
            .is_none()
    );
    assert_eq!(terminal, b"summary\n");
}
#[test]
fn https_builder_selects_crypto_without_global_initialization() {
    assert!(
        tonic::transport::Endpoint::from_static("https://example.com")
            .tls_config(tonic::transport::ClientTlsConfig::new())
            .is_ok()
    );
}

mod grpc {
    use super::*;
    use crate::{
        measurements::AdditionalMeasurements,
        proto::{
            geyser::{
                self,
                geyser_server::{Geyser, GeyserServer},
                subscribe_update::UpdateOneof,
            },
            solana::storage::confirmed_block,
        },
        providers::{GeyserProvider, ProviderContext, yellowstone::YellowstoneProvider},
    };
    use std::{
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize},
        },
        time::Instant,
    };
    use tonic::{Request, Response, Status};
    type Updates =
        Pin<Box<dyn futures::Stream<Item = Result<geyser::SubscribeUpdate, Status>> + Send>>;
    struct Mock {
        ready: Arc<tokio::sync::Barrier>,
    }
    #[tonic::async_trait]
    impl Geyser for Mock {
        type SubscribeStream = Updates;
        async fn subscribe(
            &self,
            request: Request<tonic::Streaming<geyser::SubscribeRequest>>,
        ) -> Result<Response<Self::SubscribeStream>, Status> {
            let mut requests = request.into_inner();
            let ready = Arc::clone(&self.ready);
            let (tx, rx) = tokio::sync::mpsc::channel(16);
            tokio::spawn(async move {
                let subscription = requests.message().await.unwrap().unwrap();
                assert_eq!(subscription.accounts.len(), 1);
                assert!(subscription.slots.is_empty());
                ready.wait().await;
                tx.send(Ok(geyser::SubscribeUpdate {
                    update_oneof: Some(UpdateOneof::Account(geyser::SubscribeUpdateAccount {
                        slot: 1,
                        is_startup: false,
                        account: Some(geyser::SubscribeUpdateAccountInfo {
                            pubkey: vec![1; 32],
                            txn_signature: Some(vec![2; 64]),
                            ..Default::default()
                        }),
                    })),
                    ..Default::default()
                }))
                .await
                .unwrap();
                tx.send(Ok(geyser::SubscribeUpdate {
                    update_oneof: Some(UpdateOneof::Transaction(
                        geyser::SubscribeUpdateTransaction {
                            slot: 1,
                            transaction: Some(geyser::SubscribeUpdateTransactionInfo {
                                signature: vec![2; 64],
                                transaction: Some(confirmed_block::Transaction {
                                    signatures: vec![vec![2; 64]],
                                    message: Some(confirmed_block::Message {
                                        account_keys: vec![vec![0; 32]],
                                        ..Default::default()
                                    }),
                                }),
                                ..Default::default()
                            }),
                        },
                    )),
                    ..Default::default()
                }))
                .await
                .unwrap();
            });
            Ok(Response::new(Box::pin(
                tokio_stream::wrappers::ReceiverStream::new(rx),
            )))
        }
        async fn subscribe_replay_info(
            &self,
            _: Request<geyser::SubscribeReplayInfoRequest>,
        ) -> Result<Response<geyser::SubscribeReplayInfoResponse>, Status> {
            Err(Status::unimplemented("unused"))
        }
        async fn ping(
            &self,
            _: Request<geyser::PingRequest>,
        ) -> Result<Response<geyser::PongResponse>, Status> {
            Err(Status::unimplemented("unused"))
        }
        async fn get_latest_blockhash(
            &self,
            _: Request<geyser::GetLatestBlockhashRequest>,
        ) -> Result<Response<geyser::GetLatestBlockhashResponse>, Status> {
            Err(Status::unimplemented("unused"))
        }
        async fn get_block_height(
            &self,
            _: Request<geyser::GetBlockHeightRequest>,
        ) -> Result<Response<geyser::GetBlockHeightResponse>, Status> {
            Err(Status::unimplemented("unused"))
        }
        async fn get_slot(
            &self,
            _: Request<geyser::GetSlotRequest>,
        ) -> Result<Response<geyser::GetSlotResponse>, Status> {
            Err(Status::unimplemented("unused"))
        }
        async fn is_blockhash_valid(
            &self,
            _: Request<geyser::IsBlockhashValidRequest>,
        ) -> Result<Response<geyser::IsBlockhashValidResponse>, Status> {
            Err(Status::unimplemented("unused"))
        }
        async fn get_version(
            &self,
            _: Request<geyser::GetVersionRequest>,
        ) -> Result<Response<geyser::GetVersionResponse>, Status> {
            Err(Status::unimplemented("unused"))
        }
    }
    #[tokio::test]
    async fn account_updates_and_transactions_share_a_real_local_grpc_subscription() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let incoming = futures::stream::unfold(listener, |listener| async move {
            Some((listener.accept().await.map(|(socket, _)| socket), listener))
        });
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(GeyserServer::new(Mock {
                    ready: Arc::new(tokio::sync::Barrier::new(2)),
                }))
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        let mut c = config();
        c.endpoint.retain(|e| e.kind == EndpointKind::Yellowstone);
        for endpoint in &mut c.endpoint {
            endpoint.url = format!("http://{address}");
        }
        c.config.yellowstone_accounts_bench_enabled = true;
        let comparator = Arc::new(Comparator::new());
        let measurements = Arc::new(AdditionalMeasurements::new(&c.endpoint));
        let shared_counter = Arc::new(AtomicUsize::new(0));
        let shared_shutdown = Arc::new(AtomicBool::new(false));
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
        let origin = Instant::now();
        let mut handles = Vec::new();
        for endpoint in &c.endpoint {
            let context = ProviderContext {
                shutdown_tx: shutdown_tx.clone(),
                shutdown_rx: shutdown_tx.subscribe(),
                start_wallclock_secs: crate::utils::get_current_timestamp(),
                start_instant: origin,
                comparator: Arc::clone(&comparator),
                measurements: Arc::clone(&measurements),
                signature_tx: None,
                shared_counter: Arc::clone(&shared_counter),
                shared_shutdown: Arc::clone(&shared_shutdown),
                target_transactions: Some(1),
                total_producers: 2,
                progress: None,
            };
            handles.push(YellowstoneProvider.process(endpoint.clone(), c.config.clone(), context));
        }
        for handle in handles {
            tokio::time::timeout(Duration::from_secs(5), handle)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        }
        let names = c
            .endpoint
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<_>>();
        let tx_summary = analysis::compute_run_summary(&comparator, &names);
        assert_eq!(tx_summary.total_signatures, 1);
        let extras = measurements.render(&c.config);
        assert!(extras.contains("Valid Acc"));
        assert_eq!(extras.matches("Valid common events: 1").count(), 1);
        assert!(!extras.contains("Other feed"));
        stop.send(()).unwrap();
        server.await.unwrap();
    }
}
