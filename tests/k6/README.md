# k6 black-box tests (Keydock)

This folder contains **black-box** tests executed with [Grafana k6](https://k6.io/).

Unlike the Rust integration tests in `apps/keydock/tests`, these tests run against a **real, compiled**
`keydock` process listening on a TCP port and using a real on-disk Fjall data directory.

## What this suite is (and is not)

- **Is**: end-to-end coverage of critical flows (process startup, HTTP wiring, auth, tokens, txn, metrics).
- **Is not**: a full error-matrix duplication of `apps/keydock/tests/*.rs`.
- **Is not**: a strict performance/SLO gate by default (see `load.js`, which is opt-in).

## Prerequisites

- Rust toolchain (to build `keydock`)
- Either `k6` installed **or** Docker/Podman available (the harness can run k6 inside a container)
- `curl` installed (used by the harness for readiness probing)

Arch-based distros:

```bash
sudo pacman -S k6
```

## Quick start (recommended)

Run the smoke scenario against a locally started process:

```bash
tests/k6/run-local.sh smoke
```

Run all scenarios (smoke + security + regression + a short load baseline):

```bash
tests/k6/run-local.sh all
```

If you don't have `k6` installed, you can run it via Docker:

```bash
K6_MODE=docker tests/k6/run-local.sh smoke
```

Run all scenarios via Docker:

```bash
K6_MODE=docker tests/k6/run-local.sh all
```

On Linux, Docker mode uses `--network host` by default so the container can hit `127.0.0.1:<PORT>`.

If you already have a Keydock service running elsewhere, you can skip starting a local process:

```bash
START_KEYDOCK=0 KEYDOCK_BASE_URL="http://127.0.0.1:8080" tests/k6/run-local.sh smoke
```

Run all scenarios against an existing service:

```bash
START_KEYDOCK=0 KEYDOCK_BASE_URL="http://127.0.0.1:8080" K6_MODE=docker tests/k6/run-local.sh all
```

If you already have a k6 container running (for example via docker-compose) with this repo mounted at `/work`,
you can reuse it:

```bash
K6_MODE=docker-exec K6_CONTAINER="k6" K6_WORKDIR="/work" tests/k6/run-local.sh smoke
```

Notes for `docker-exec`:

- The container must already be **running**.
- It must have the repo mounted so `tests/k6/scenarios/*.js` is visible under `K6_WORKDIR`.

## Scenarios

- `smoke`: single deterministic end-to-end flow (mandatory)
- `security`: auth/authz contract checks (no load)
- `regression`: light concurrency + isolation checks
- `load`: opt-in baseline load (not meant to run on every CI job)
- `all`: runs multiple scenarios in sequence (configurable via `ALL_SCENARIOS`)

Each scenario tags requests with stable `name`, `scenario`, and `flow` values. Dynamic bucket IDs and
keys stay out of metric names, so k6 can aggregate timings by operation instead of creating one metric
series per generated URL.

## Reports

Every run prints a short scenario summary and writes two files by default:

- `tests/k6/results/<scenario>-<RUN_ID>.summary.txt`: human-readable summary for local runs and CI logs.
- `tests/k6/results/<scenario>-<RUN_ID>.summary.json`: raw k6 summary data for later inspection.

`tests/k6/results` is intentionally git-ignored. In Docker mode, the harness mounts only this directory
as writable while keeping the repository mount read-only by default.

## Environment variables

The harness supports a few knobs:

- `PORT`: port to bind (default: `18080`)
- `RUN_ID`: identifier used to uniquify test data (default: generated)
- `K6_MODE`: `auto|local|docker` (default: `auto`)
- `START_KEYDOCK`: `1|0` (default: `1`)
- `KEYDOCK_BASE_URL`: required when `START_KEYDOCK=0`
- `WAIT_READY`: `1|0` (default: `1`)
- `K6_CLEANUP`: `1|0|true|false` (default: `false`). When false, scenarios skip bucket deletion at the end.
- `ALL_SCENARIOS`: space-separated list for `all` (default: `smoke security regression load`)
- `K6_BIN`: override k6 binary (default: `k6`)
- `DOCKER_BIN`: override docker binary (default: `docker`) — set to `podman` if desired
- `K6_IMAGE`: docker image for k6 (default: `grafana/k6:latest`)
- `K6_NETWORK`: docker network mode (default: `host`)
- `K6_MOUNT`: repo mount mode into container (`ro|rw`, default: `ro`)
- `K6_DOCKER_USER`: Docker user used for writing summaries (default: current `uid:gid`)
- `K6_CONTAINER`: existing container name/id when `K6_MODE=docker-exec`
- `K6_WORKDIR`: workdir inside `K6_CONTAINER` (default: `/work`)
- `CURL_BIN`: override curl binary (default: `curl`)
- `CARGO_BIN`: override cargo binary (default: `cargo`)
- `KEYDOCK_BIN`: override built binary path (default: `target/release/keydock`)
- `K6_SUMMARY_DIR`: relative output directory for summaries (default: `tests/k6/results`)

### Useful variations

Only run a subset with `all`:

```bash
ALL_SCENARIOS="smoke security" tests/k6/run-local.sh all
```

Override the load baseline when running `all`:

```bash
LOAD_VUS=10 LOAD_DURATION=30s tests/k6/run-local.sh all
```

Enable cleanup at the end of each scenario:

```bash
K6_CLEANUP=1 tests/k6/run-local.sh smoke
```

Write summaries to another relative directory:

```bash
K6_SUMMARY_DIR=.local/k6-results tests/k6/run-local.sh smoke
```

## Notes on determinism

- Tests create random/unique buckets/keys per run (`RUN_ID` + per-VU/per-iter nonce) to avoid shared state.
- The harness uses a temporary `--data-dir` and removes it at the end.
- Scripts must not log secrets or bearer tokens.
- When running against an existing persistent service (`START_KEYDOCK=0`), enable `K6_CLEANUP=1` to delete scenario buckets on exit.
