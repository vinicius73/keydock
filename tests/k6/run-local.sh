#!/usr/bin/env bash
set -Eeuo pipefail

readonly DEFAULT_ALL_SCENARIOS="smoke security regression contracts load"
readonly READY_TIMEOUT_SECONDS=25

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd -P)"
cd "${REPO_ROOT}"

usage() {
  cat <<'EOF'
Usage:
  tests/k6/run-local.sh <scenario>

Scenarios:
  smoke | security | regression | contracts | load | stress | all

Environment variables:
  PORT        Port to bind (default: 18080)
  RUN_ID      Identifier used to uniquify test data (default: generated)
  START_KEYDOCK  1|0 (default: 1). When 0, run against an existing service.
  KEYDOCK_BASE_URL  Base URL when START_KEYDOCK=0 (e.g. http://127.0.0.1:8080)
  KEYDOCK_ROOT_KEY  Optional server root key when START_KEYDOCK=1 (default: generated)
  WAIT_READY  1|0 (default: 1). When 1, probes GET /ready before running k6.
  K6_CLEANUP  1|0|true|false (default: false). When false, do not delete buckets at end of scenarios.
  ALL_SCENARIOS  Space-separated list when scenario=all (default: "smoke security regression contracts load")
  LOAD_VUS    VUs for load scenario (default: 10; forced to 2 when running via all)
  LOAD_DURATION  Duration for load scenario (default: 30s; forced to 2s when running via all)
  STRESS_MAX_VUS  Peak VUs for stress scenario (default: 40)
  STRESS_RAMP_UP  Ramp up duration for stress scenario (default: 15s)
  STRESS_HOLD     Hold duration for stress scenario (default: 15s)
  STRESS_RAMP_DOWN  Ramp down duration for stress scenario (default: 10s)
  STRESS_ABORT_TRANSPORT_ERRORS  Abort stress after N transport errors in a row (default: 25)
  K6_MODE     auto|local|docker|docker-exec (default: auto)
  K6_BIN      Override k6 binary (default: k6)
  DOCKER_BIN  Override docker binary (default: docker)
  K6_IMAGE    k6 docker image when K6_MODE=docker (default: grafana/k6:latest)
  K6_NETWORK  Docker network mode for k6 (default: host)
  K6_MOUNT    ro|rw mount for repo into k6 container (default: ro)
  K6_DOCKER_USER  Docker user for k6 file outputs (default: current uid:gid)
  K6_CONTAINER  Existing container name/id when K6_MODE=docker-exec
  K6_WORKDIR    Workdir inside K6_CONTAINER (default: /work)
  CURL_BIN    Override curl binary (default: curl)
  CARGO_BIN   Override cargo binary (default: cargo)
  KEYDOCK_BIN Override built binary path (default: target/release/keydock)
  K6_ARGS     Extra args forwarded to `k6 run` (simple whitespace split, no shell evaluation)
  K6_SUMMARY_DIR  Relative directory for per-scenario summaries (default: tests/k6/results)
EOF
}

die() {
  echo "error: $*" >&2
  exit 2
}

info() {
  echo "$*"
}

has_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1
}

require_cmd() {
  local name="$1"
  local cmd="$2"
  if ! has_cmd "$cmd"; then
    echo "error: missing dependency: ${name} (${cmd})" >&2
    if [[ "${name}" == "k6" ]]; then
      echo "hint: install k6 (see https://k6.io/docs/get-started/installation/)" >&2
      echo "hint: on Arch-based distros: sudo pacman -S k6" >&2
      echo "hint: or run via Docker: K6_MODE=docker tests/k6/run-local.sh <scenario>" >&2
    fi
    if [[ "${name}" == "docker" ]]; then
      echo "hint: install Docker (or set DOCKER_BIN=podman if using Podman)" >&2
    fi
    exit 2
  fi
}

docker_container_running() {
  local container="$1"
  [[ "$("${DOCKER_BIN}" inspect -f '{{.State.Running}}' -- "${container}" 2>/dev/null || true)" == "true" ]]
}

require_env() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    die "missing required env var: ${name}"
  fi
}

validate_bool() {
  local name="$1"
  local value="$2"

  case "${value}" in
    0 | 1) ;;
    *) die "${name} must be 0 or 1 (got '${value}')" ;;
  esac
}

validate_cleanup_value() {
  local value="$1"

  case "${value}" in
    0 | 1 | false | true) ;;
    *) die "K6_CLEANUP must be 0, 1, false, or true (got '${value}')" ;;
  esac
}

validate_port() {
  local value="$1"

  if [[ ! "${value}" =~ ^[0-9]+$ ]] || (( value < 1 || value > 65535 )); then
    die "PORT must be an integer between 1 and 65535 (got '${value}')"
  fi
}

validate_run_id() {
  local value="$1"

  if [[ ! "${value}" =~ ^[A-Za-z0-9._-]+$ ]]; then
    die "RUN_ID may only contain letters, numbers, dots, underscores, and hyphens"
  fi
}

validate_summary_dir() {
  local value="$1"

  if [[ -z "${value}" ]]; then
    die "K6_SUMMARY_DIR cannot be empty"
  fi
  if [[ "${value}" = /* ]]; then
    die "K6_SUMMARY_DIR must be relative to the repository root"
  fi
  case "${value}" in
    .. | ../* | */.. | */../*) die "K6_SUMMARY_DIR cannot contain '..' path segments" ;;
  esac
}

validate_scenario_name() {
  local value="$1"

  case "${value}" in
    smoke | security | regression | contracts | load | stress | all) ;;
    *)
      die "unknown scenario '${value}' (expected: smoke|security|regression|contracts|load|stress|all)"
      ;;
  esac
}

parse_words() {
  local -n target="$1"
  local raw="$2"

  target=()
  if [[ -n "${raw}" ]]; then
    local IFS=$' \t\n'
    read -r -a target <<<"${raw}"
  fi
}

cleanup() {
  local exit_code="$?"
  trap - EXIT INT TERM
  set +e

  if [[ -n "${KEYDOCK_PID:-}" ]]; then
    kill -TERM "${KEYDOCK_PID}" >/dev/null 2>&1 || true
    for _ in {1..50}; do
      if ! kill -0 "${KEYDOCK_PID}" >/dev/null 2>&1; then
        break
      fi
      sleep 0.1
    done
    kill -KILL "${KEYDOCK_PID}" >/dev/null 2>&1 || true
    wait "${KEYDOCK_PID}" >/dev/null 2>&1 || true
  fi

  if [[ -n "${READY_BODY_FILE:-}" && -f "${READY_BODY_FILE}" ]]; then
    rm -f -- "${READY_BODY_FILE}"
  fi

  if [[ -n "${TMP_DIR:-}" && -d "${TMP_DIR}" ]]; then
    if (( exit_code != 0 )) && [[ -n "${LOG_PATH:-}" && -f "${LOG_PATH}" ]]; then
      echo "keydock log (${LOG_PATH}):" >&2
      sed -n '1,160p' "${LOG_PATH}" >&2 || true
    fi
    rm -rf -- "${TMP_DIR}"
  fi

  exit "${exit_code}"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

scenario="${1:-}"
if [[ -z "${scenario}" || "${scenario}" == "-h" || "${scenario}" == "--help" ]]; then
  usage
  exit 0
fi
validate_scenario_name "${scenario}"

PORT="${PORT:-18080}"
RUN_ID="${RUN_ID:-k6-$(date +%s)-$$}"
START_KEYDOCK="${START_KEYDOCK:-1}"
WAIT_READY="${WAIT_READY:-1}"
K6_CLEANUP="${K6_CLEANUP:-false}"

K6_MODE="${K6_MODE:-auto}"
K6_BIN="${K6_BIN:-k6}"
DOCKER_BIN="${DOCKER_BIN:-docker}"
K6_IMAGE="${K6_IMAGE:-grafana/k6:latest}"
K6_NETWORK="${K6_NETWORK:-host}"
K6_MOUNT="${K6_MOUNT:-ro}"
K6_DOCKER_USER="${K6_DOCKER_USER:-$(id -u):$(id -g)}"
K6_CONTAINER="${K6_CONTAINER:-}"
K6_WORKDIR="${K6_WORKDIR:-/work}"
K6_SUMMARY_DIR="${K6_SUMMARY_DIR:-tests/k6/results}"
CURL_BIN="${CURL_BIN:-curl}"
CARGO_BIN="${CARGO_BIN:-cargo}"
KEYDOCK_BIN="${KEYDOCK_BIN:-target/release/keydock}"

validate_port "${PORT}"
validate_run_id "${RUN_ID}"
validate_bool "START_KEYDOCK" "${START_KEYDOCK}"
validate_bool "WAIT_READY" "${WAIT_READY}"
validate_cleanup_value "${K6_CLEANUP}"
validate_summary_dir "${K6_SUMMARY_DIR}"

case "${K6_MOUNT}" in
  ro | rw) ;;
  *) die "K6_MOUNT must be ro or rw (got '${K6_MOUNT}')" ;;
esac

require_cmd "curl" "${CURL_BIN}"

pick_k6_mode() {
  case "${K6_MODE}" in
    local)
      require_cmd "k6" "${K6_BIN}"
      info "Using local k6 (${K6_BIN})"
      ;;
    docker)
      require_cmd "docker" "${DOCKER_BIN}"
      info "Using k6 via Docker (${K6_IMAGE})"
      ;;
    docker-exec)
      require_cmd "docker" "${DOCKER_BIN}"
      if [[ -z "${K6_CONTAINER}" ]]; then
        die "K6_MODE=docker-exec requires K6_CONTAINER"
      fi
      if ! docker_container_running "${K6_CONTAINER}"; then
        echo "error: K6_CONTAINER='${K6_CONTAINER}' is not running (cannot docker exec)" >&2
        echo "hint: start it first, and ensure the repo is mounted at ${K6_WORKDIR}" >&2
        exit 2
      fi
      info "Using k6 via docker exec (container=${K6_CONTAINER})"
      ;;
    auto)
      if has_cmd "${K6_BIN}"; then
        K6_MODE="local"
        info "Using local k6 (${K6_BIN})"
      elif [[ -n "${K6_CONTAINER}" ]] && has_cmd "${DOCKER_BIN}" && docker_container_running "${K6_CONTAINER}"; then
        K6_MODE="docker-exec"
        info "Using k6 via docker exec (container=${K6_CONTAINER})"
      elif has_cmd "${DOCKER_BIN}"; then
        K6_MODE="docker"
        info "Using k6 via Docker (${K6_IMAGE})"
      else
        echo "error: neither k6 nor docker is available" >&2
        echo "hint: install k6 (sudo pacman -S k6) OR install Docker" >&2
        echo "hint: if using Podman, set DOCKER_BIN=podman and K6_MODE=docker" >&2
        exit 2
      fi
      ;;
    *)
      die "invalid K6_MODE='${K6_MODE}' (expected: auto|local|docker|docker-exec)"
      ;;
  esac
}

k6_run() {
  local script_path="$1"
  local -a k6_args=()
  parse_words k6_args "${K6_ARGS:-}"

  case "${K6_MODE}" in
    local)
      "${K6_BIN}" run "${k6_args[@]}" "${script_path}"
      ;;
    docker)
      "${DOCKER_BIN}" run --rm \
        --network "${K6_NETWORK}" \
        --user "${K6_DOCKER_USER}" \
        -e KEYDOCK_BASE_URL \
        -e RUN_ID \
        -e K6_CLEANUP \
        -e K6_SUMMARY_JSON \
        -e K6_SUMMARY_TEXT \
        -e LOAD_VUS \
        -e LOAD_DURATION \
        -e STRESS_MAX_VUS \
        -e STRESS_RAMP_UP \
        -e STRESS_HOLD \
        -e STRESS_RAMP_DOWN \
        -e STRESS_ABORT_TRANSPORT_ERRORS \
        -v "${PWD}:/work:${K6_MOUNT}" \
        -v "${PWD}/${K6_SUMMARY_DIR}:/work/${K6_SUMMARY_DIR}:rw" \
        -w /work \
        "${K6_IMAGE}" \
        run "${k6_args[@]}" "${script_path}"
      ;;
    docker-exec)
      "${DOCKER_BIN}" exec \
        -e KEYDOCK_BASE_URL \
        -e RUN_ID \
        -e K6_CLEANUP \
        -e K6_SUMMARY_JSON \
        -e K6_SUMMARY_TEXT \
        -e LOAD_VUS \
        -e LOAD_DURATION \
        -e STRESS_MAX_VUS \
        -e STRESS_RAMP_UP \
        -e STRESS_HOLD \
        -e STRESS_RAMP_DOWN \
        -e STRESS_ABORT_TRANSPORT_ERRORS \
        -w "${K6_WORKDIR}" \
        "${K6_CONTAINER}" \
        k6 run "${k6_args[@]}" "${script_path}"
      ;;
    *)
      die "internal: unsupported K6_MODE='${K6_MODE}'"
      ;;
  esac
}

prepare_summary_paths() {
  local scenario_name="$1"

  mkdir -p -- "${K6_SUMMARY_DIR}"
  K6_SUMMARY_TEXT="${K6_SUMMARY_DIR}/${scenario_name}-${RUN_ID}.summary.txt"
  K6_SUMMARY_JSON="${K6_SUMMARY_DIR}/${scenario_name}-${RUN_ID}.summary.json"
  export K6_SUMMARY_TEXT K6_SUMMARY_JSON

  info "k6 summary: ${K6_SUMMARY_TEXT}"
  info "k6 raw summary: ${K6_SUMMARY_JSON}"
}

pick_k6_mode

if [[ "${START_KEYDOCK}" == "1" ]]; then
  require_cmd "cargo" "${CARGO_BIN}"

  TMP_DIR="$(mktemp -d -t keydock-k6.XXXXXX)"
  DATA_DIR="${TMP_DIR}/data"
  LOG_PATH="${TMP_DIR}/keydock.log"
  mkdir -p "${DATA_DIR}"

  info "Building keydock (release)..."
  "${CARGO_BIN}" build -p keydock --release

  if [[ ! -x "${KEYDOCK_BIN}" ]]; then
    die "keydock binary not found/executable at '${KEYDOCK_BIN}'"
  fi

  ADDR="127.0.0.1:${PORT}"
  BASE_URL="http://${ADDR}"
  SERVER_ROOT_KEY="${KEYDOCK_ROOT_KEY:-keydock-k6-${RUN_ID}-root-key}"

  info "Starting keydock at ${BASE_URL}..."
  KEYDOCK_ROOT_KEY="${SERVER_ROOT_KEY}" "${KEYDOCK_BIN}" serve --listen "${ADDR}" --data-dir "${DATA_DIR}" >"${LOG_PATH}" 2>&1 &
  KEYDOCK_PID="$!"
else
  require_env "KEYDOCK_BASE_URL"
  BASE_URL="${KEYDOCK_BASE_URL%/}"
  case "${BASE_URL}" in
    http://* | https://*) ;;
    *) die "KEYDOCK_BASE_URL must start with http:// or https://" ;;
  esac
  info "Using existing keydock at ${BASE_URL}"
fi

wait_ready() {
  local url="${BASE_URL}/ready"
  local deadline="$((SECONDS + READY_TIMEOUT_SECONDS))"
  local http_code
  if [[ -n "${TMP_DIR:-}" ]]; then
    READY_BODY_FILE="${TMP_DIR}/ready.json"
  else
    READY_BODY_FILE="$(mktemp -t keydock-k6-ready.XXXXXX)"
  fi

  while (( SECONDS < deadline )); do
    http_code="$("${CURL_BIN}" --silent --max-time 2 --output "${READY_BODY_FILE}" --write-out '%{http_code}' "${url}" 2>/dev/null || true)"
    if [[ "${http_code}" == "200" ]]; then
      return 0
    fi
    sleep 0.2
  done

  echo "error: server did not become ready at ${url} within timeout" >&2
  if [[ -n "${LOG_PATH:-}" ]]; then
    echo "hint: server log follows before cleanup" >&2
  fi
  return 1
}

if [[ "${WAIT_READY}" == "1" ]]; then
  wait_ready
fi

export KEYDOCK_BASE_URL="${BASE_URL}"
export RUN_ID
export K6_CLEANUP

run_scenario() {
  local scenario_name="$1"
  local script_path="tests/k6/scenarios/${scenario_name}.js"

  validate_scenario_name "${scenario_name}"
  if [[ "${scenario_name}" == "all" ]]; then
    die "ALL_SCENARIOS cannot include 'all'"
  fi

  if [[ ! -f "${script_path}" ]]; then
    die "missing scenario script: ${script_path}"
  fi

  if [[ "${scenario_name}" == "load" ]]; then
    # Keep `all` reasonably fast by default. Override via env if desired.
    LOAD_VUS="${LOAD_VUS:-2}"
    LOAD_DURATION="${LOAD_DURATION:-2s}"
    export LOAD_VUS LOAD_DURATION
  fi

  prepare_summary_paths "${scenario_name}"
  info "Running k6 scenario '${scenario_name}'..."
  k6_run "${script_path}"
}

if [[ "${scenario}" == "all" ]]; then
  ALL_SCENARIOS="${ALL_SCENARIOS:-${DEFAULT_ALL_SCENARIOS}}"
  declare -a SCENARIOS=()
  parse_words SCENARIOS "${ALL_SCENARIOS}"
  if [[ "${#SCENARIOS[@]}" -eq 0 ]]; then
    die "ALL_SCENARIOS is empty"
  fi

  for selected_scenario in "${SCENARIOS[@]}"; do
    run_scenario "${selected_scenario}"
  done
else
  run_scenario "${scenario}"
fi

