set dotenv-load := true

export RUST_LOG := env_var_or_default('RUST_LOG', 'info')
export RUST_BACKTRACE := env_var_or_default('RUST_BACKTRACE', '0')
export PATH := env_var('HOME') + '/.cargo/bin:' + env_var('PATH')

# Default: show available recipes
default:
    @just --list

setup: install-rust
    rustup update
    # Remove any directory override and let rust-toolchain.toml take precedence
    rustup override unset || true
    # Force rustup to detect and install toolchain from rust-toolchain.toml if needed
    # Running cargo will make rustup automatically install the toolchain if not installed
    @cargo --version >/dev/null


# --- QA loop (repo convention) ---
# Quick usage:
# - Full QA gate: `just qa`
# - Iterate on one package: `just qa keydock-http` (or `just test keydock-http`)
# - Full workspace tests (final gate): `just test`
#
# Package mapping (when you want targeted iteration):
# - HTTP routes / OpenAPI: `keydock` (integration tests live under `apps/keydock/tests`)
# - Domain rules: `keydock-domain`
# - Use cases / ports: `keydock-usecase`
# - Storage adapter: `keydock-fjall`

[group('qa')]
fmt:
    cargo fmt --all

[group('qa')]
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

[group('qa')]
check:
    cargo check --workspace --all-targets

[group('qa')]
fix: fmt
    cargo clippy --workspace --all-targets --fix --allow-dirty

[group('qa')]
test pkg="":
    if [ -n "{{ pkg }}" ]; then \
      cargo test -p "{{ pkg }}"; \
    else \
      cargo test --workspace; \
    fi

[group('qa')]
qa pkg="": fix
    just test {{ pkg }}

[group('qa')]
k6 scenario="all":
    tests/k6/run-local.sh {{ scenario }}


# --- Dev ---

[group('dev')]
run +ARGS="":
    cargo run -p keydock -- {{ ARGS }}

[group('dev')]
serve +ARGS="":
    cargo run -p keydock -- serve {{ ARGS }}

[group('dev')]
release-serve +ARGS="":
    cargo build -p keydock --release
    if [ ! -f .local/keydock.toml ]; then target/release/keydock init .local; fi
    target/release/keydock serve -c .local/keydock.toml {{ ARGS }}

[group('dev')]
serve-watch +ARGS="":
    cargo watch -x 'run -p keydock -- serve {{ ARGS }}'


# --- Build / utilities ---

[group('build')]
build:
    cargo build --workspace

[group('build')]
release:
    cargo build --workspace --release

[group('build')]
clean:
    cargo clean

[group('tools')]
nextest:
    cargo nextest run --workspace

[group('tools')]
cov:
    cargo llvm-cov --workspace --all-targets --lcov --output-path target/lcov.info

[private]
install-rust:
    command -v rustup >/dev/null || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal
    rustup toolchain install --profile minimal --component clippy --component rustfmt --component rust-analyzer
    cargo binstall --help >/dev/null || curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
    rustc --version
    cargo --version
    cargo binstall -V
