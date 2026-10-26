# Solana Stream Bench

Solana Stream Bench is based directly on [solstackapp/geyserbench](https://github.com/solstackapp/geyserbench). It retains upstream's transaction benchmarking and provider implementations, with optional Yellowstone gRPC account comparisons and local TXT reports. See [UPSTREAM.md](UPSTREAM.md) for the exact source revision and local changes, and [LICENSE](LICENSE) for the upstream MIT license.

## Build and run

```bash
cargo build --release
# If config.toml is missing, the first run creates it and exits.
# Edit its endpoint URLs and optional tokens before starting the benchmark.
cargo run --release
# Or:
./target/release/solana-stream-bench --config /path/to/config.toml
```

The default file is `config.toml`. If missing, a template is created and the program exits so you can edit it. Rust 1.97.1 is selected by the repository toolchain file. Building the retained UDP implementation also requires the native dependencies used by Solana ledger (including a C/C++ compiler and libclang). Protobuf compilation uses the vendored protoc executable.

If upgrading from the previous implementation, the schema has changed: `transactions` sets the shared transaction target, and each endpoint needs a `kind`. The old `[statistics]` and `[grpc]` tuning sections are no longer used. Keep credentials in the ignored local config. A previous local config is retained as `config.toml.previous` when migrated during this rewrite.

## Configuration

Configure the generated `config.toml` using this example:

```toml
[config]
transactions = 1000
account = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA"
commitment = "processed"
yellowstone_accounts_bench_enabled = true

[output]
directory = "./results"
timestamp_format = "%Y-%m-%d_%H-%M-%S"
txt_file_output = true

[backend]
enabled = false

[[endpoint]]
name = "Provider A"
url = "https://provider-a.example.com:443"
kind = "yellowstone"
# x_token = "YOUR_TOKEN"

[[endpoint]]
name = "Provider B"
url = "https://provider-b.example.com:443"
kind = "yellowstone"
```

Addresses are placeholders. Add any number of `[[endpoint]]` sections with unique names. Use `kind = "yellowstone"` for Yellowstone gRPC endpoints. Upstream's `arpc`, `arpcv3`, `thor`, `shredstream`, `shreder`, `jetstream`, and `udpshredstream` types remain available for transaction comparison. Use HTTP/HTTPS URLs for gRPC feeds; the raw UDP receiver takes a bare bind address such as `0.0.0.0:20000`. Authentication tokens are optional.

`config.transactions` is the number of matching transaction signatures observed by **every configured endpoint**, following upstream's collection semantics. `0` runs until Ctrl+C. Negative values are rejected. Transactions are filtered for inclusion of the public key in `config.account`; `config.commitment` is `processed`, `confirmed`, or `finalized`.

Account measurements share the transaction benchmark's lifetime. They do not independently terminate the run. Ctrl+C stops collection and writes a summary of the partial observations. Upstream connection-setup errors terminate the process before a final report is available. A stream that disconnects or does not observe the same transactions can prevent the common transaction target being reached; stop with Ctrl+C to inspect available data.

## Conditional account updates

`yellowstone_accounts_bench_enabled = true` enables account subscriptions **only on Yellowstone gRPC endpoints**. The subscription filters for accounts owned by the program public key in `config.account`. This is the same required public key used for transaction inclusion filtering; it is also required when account benchmarking is disabled.

Only non-startup updates carrying a transaction signature are used. Events match by `(slot, account public key, transaction signature)`. Validator-local write versions are deliberately excluded. Multiple writes to the same account within one transaction share a key; the earliest notification is retained.

The account summary has the same shape as upstream's transaction table: Endpoint, First %, P50 ms, P95 ms, P99 ms, Valid Acc, Firsts, and Backfill. Only gRPC providers appear in it. Valid updates are the intersection observed by **all configured gRPC providers**, independently of other feed kinds.

## Comparison tables

All providers appear side by side in one transaction table. Account tables use only the gRPC subset. The underlying summary logic is upstream's:

- Only events common to every provider participating in that table count as valid.
- Delay is time behind the earliest observed arrival for that event, measured with a monotonic clock.
- First % and Firsts indicate first detections; P50/P95/P99 summarize relative delay, including zero for first arrivals.
- Exact timestamp ties retain upstream's single-winner behavior; no fractional tie credit is added.
- Percentiles use upstream's rounded index into the sorted samples (`round(p*(n-1))`).
- The Backfill column follows upstream's wall-clock comparison with run start. Additional categories capture reception time locally and do not estimate historical event age, so their Backfill count is normally zero.

Account timestamps are captured immediately after message reception, before parsing identifiers. The transaction path now uses that same early timestamp. Protobuf decoding and local scheduling still affect measurements. A single gRPC endpoint can produce a table, but meaningful comparative latency requires at least two.

Account events are retained for the duration of the benchmark, with repeated event IDs keeping the earliest observation.

## TXT output

Every completed or interrupted collection writes a UTC timestamped report by default:

```text
results/pAMM_fXEA_2026-10-26_03-19-00.txt
```

The filename prefix is generated from the first four and last four characters of `config.account`, separated by an underscore (for example, `pAMM_fXEA`). The timestamp uses UTC run start time. The directory is created automatically; collisions append `_1`, `_2`, etc., using exclusive file creation. `output.txt_file_output = false` keeps terminal output without a file. The timestamp format must produce a safe filename component.

The final summary is rendered once, then the **same bytes** are written to stdout and TXT, including trailing newlines. It includes metadata, the transaction table, and the optional gRPC account table. No local JSON result file is generated. Progress and connection logs go to stderr; terminal escape sequences are not included in the report.

## Optional upstream backend

Upstream's external backend integration is retained, but defaults to **disabled** for local runs. Set `[backend] enabled = true` to explicitly publish transaction observations to the upstream SolStack service, or configure your own `backend.url`. `--private` always disables publishing. Account observations stay local and are not added to the backend payload. Upstream's high-volume cutoff still disables streaming above 100,000 transactions.

## Development

```bash
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets
cargo build --release
cargo run --release -- --help
```

Tests cover optional subscription filters, gRPC-only event comparisons, matching identifiers, account event retention, TXT collision handling and byte equality, and a localhost gRPC stream. Successful local tests do not imply live-provider verification.

## Attribution

The base implementation is Copyright (c) 2025 SolStack.app, under MIT. Required upstream license text is retained in LICENSE. Provider protocol identifiers and third-party package names remain upstream names.
