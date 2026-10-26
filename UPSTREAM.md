# Source provenance

- Repository: https://github.com/solstackapp/geyserbench
- Copied revision: `784b40cfd623760a24682c352ab90537182ca3f5`
- License: MIT; the original copyright and permission notice are retained in LICENSE.

Solana Stream Bench uses this source revision as its replacement base, including all provider implementations, protobuf sources, local Yellowstone client, deshredding, upstream transaction comparison logic, and optional backend integration.

Local changes:

- Rename the package, executable, CLI, documentation and packaging to Solana Stream Bench / `solana-stream-bench`.
- Add optional account subscriptions to the Yellowstone provider only. Capture monotonic arrival time before identifier parsing. Other providers receive an unused additional-measurement context field; their transaction implementations remain upstream.
- Use the required `account` configuration key for transaction inclusion filtering and, when enabled, Yellowstone account-owner filtering.
- Compare account keys by slot/public key/transaction signature, excluding validator-local write versions. Reuse upstream summary statistics across the gRPC subset.
- Retain account observations for the duration of the benchmark.
- Render summaries once into a string; write exactly that string to stdout and one exclusive, timestamped TXT file. Preserve partial summaries on Ctrl+C. Remove the unused local JSON metrics formatter.
- Render every provider row even when samples are unavailable, and show the fastest provider's actual median rather than a hard-coded zero.
- Default external backend publishing to disabled, honor its configured URL, and retain `--private` as an override. The original service integration remains available by explicit configuration; account observations remain local.
- Enable Tonic's Ring TLS provider explicitly, and apply TLS to HTTPS endpoints while allowing HTTP localhost streams.
- Validate configuration, scaffold placeholder endpoints, and stop after creating a missing config so users can enter their own addresses.
- Add automated tests and local documentation. Retain the upstream MIT license notice.

No live-provider benchmarking or external backend publishing is performed as part of automated verification.
