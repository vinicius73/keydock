# Reference — Rust planning, gates, QA (keydock)

This reference exists to keep “planning” concrete and aligned with how this repository is actually structured and validated.

## Sources of truth (repo rules)

- `.cursor/rules/project-structure.mdc` (layer locations + route ↔ test mapping)
- `.cursor/rules/rust-qa-loop.mdc` (QA commands and targeted test mapping)
- `.cursor/rules/error-handling.mdc` (typed errors + HTTP boundary mapping)
- `.cursor/rules/testing-e2e.mdc` (HTTP integration tests; complete response shape assertions)
- `.cursor/rules/rust-code-style.mdc` + `.cursor/rules/rust-style-imports.mdc`

## Where code lives (high-signal map)

- Composition root: `apps/keydock/src/main.rs`
- HTTP edge (Axum): `crates/keydock-http/`
  - Router builder: `crates/keydock-http/src/router.rs`
  - Routes: `crates/keydock-http/src/routes/*.rs`
  - OpenAPI: `crates/keydock-http/src/openapi.rs`
- Domain: `crates/keydock-domain/`
- Use cases / ports: `crates/keydock-usecase/`
- Storage adapter: `crates/keydock-fjall/`
- Shared HTTP state: `crates/keydock-state/`
- Config / CLI: `crates/keydock-config/`
- Testkit: `crates/keydock-testkit/`
- HTTP integration tests: `apps/<app>/tests/*.rs` (today: mostly `apps/keydock/tests/`)

## Cross-cutting gates (mandatory)

- No warnings: `cargo clippy --workspace --all-targets` must pass cleanly.
- Production safety: avoid `unwrap()` / `expect()` in non-test code (see `.cursor/rules/error-handling.mdc`).
- HTTP contract safety: integration tests must assert the **complete response shape you care about** (see `.cursor/rules/testing-e2e.mdc`).
- Imports: keep imports at the top and group `std → external → internal` (see `.cursor/rules/rust-style-imports.mdc`).

## QA loop (copy/paste)

### While iterating (targeted)

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test -p <package>
```

Use the package mapping from `.cursor/rules/rust-qa-loop.mdc` to pick `<package>` based on what you changed.

### Before claiming “done” (workspace)

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

## When to prioritize `cargo test --workspace` earlier

Run `cargo test --workspace` earlier (not only at the end) when:

- You changed HTTP routes / OpenAPI (`crates/keydock-http/` or `apps/keydock/`)
- You changed request/response DTO shapes or error bodies
- You changed auth/authz or any tenant isolation behavior
- You changed storage semantics (`crates/keydock-fjall/`) or broad fan-out crates (`keydock-state`, `keydock-config`)

## Planning examples (good)

### Example A — route change + integration test (high risk)

**Phase 1**

- Objective: Add `?include_deleted=true` to a list endpoint and include deleted items only when requested.
- Where:
  - `crates/keydock-http/src/routes/<area>.rs` (query parsing + wiring)
  - `crates/keydock-usecase/src/<area>/...` (use case contract)
  - `apps/keydock/tests/<area>_*.rs` (integration test: full JSON success + error paths)
- Decisions:
  - Default behavior: omitted param preserves existing output.
  - Validation: invalid query value maps to the established HTTP error shape (per `error-handling.mdc`).
  - Testing: assert the complete response JSON shape (including nullables/metadata when present).
- QA plan:
  - `cargo fmt --all`
  - `cargo clippy --workspace --all-targets`
  - `cargo test -p keydock`

### Example B — domain validation change (localized)

**Phase 1**

- Objective: Tighten key name validation to reject trailing whitespace.
- Where:
  - `crates/keydock-domain/src/...`
  - In-file unit tests (`#[cfg(test)] mod tests`) for the validation function
- QA plan:
  - `cargo fmt --all`
  - `cargo clippy --workspace --all-targets`
  - `cargo test -p keydock-domain`
