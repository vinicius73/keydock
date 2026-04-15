# Reference — Refining Rust tests (unit + HTTP integration)

This document expands the repo-specific directives used by the `refine-tests` skill.

## Applicable repo rules (source of truth)

- `.cursor/rules/project-structure.mdc` (route ↔ test mapping, locations)
- `.cursor/rules/testing-unit.mdc` (unit test organization, helpers, fixtures)
- `.cursor/rules/rust-style-testing.mdc` (mandatory testing patterns)
- `.cursor/rules/testing-e2e.mdc` (**CRITICAL**: complete response verification)
- `.cursor/rules/rust-qa-loop.mdc` (validation workflow: fmt → clippy → tests)

## Primary goal

Refine the provided test files by:

- Removing tests that are **irrelevant** (low regression value)
- Consolidating tests that are **redundant** (same behavior / same assertions)
- Keeping or improving **signal quality** (failures should point to real regressions)
- Preserving determinism and minimizing diff

## Mandatory compliance targets (what to detect + fix)

When refining tests, you are also responsible for bringing the touched files back into compliance with repo rules.

### Rust assertions (unit + integration)

- MUST use `pretty_assertions::assert_eq` for equality assertions.
- MUST convert `assert!(cond)` to `assert_eq!(cond, true, "...")` to preserve readable diffs and consistent style.

Detectors:

- `assert!(` appears in a Rust test file.
- Equality assertions exist but `use pretty_assertions::assert_eq;` is missing.

Fix recipes:

- Replace:
  - `assert!(condition);`
  - `assert!(condition, "msg");`
  - `assert!(condition, "msg {}", x);`
    with:
  - `assert_eq!(condition, true);`
  - `assert_eq!(condition, true, "msg");`
  - `assert_eq!(condition, true, "msg {}", x);`
    and ensure `use pretty_assertions::assert_eq;` is present in the test module.

### Rust imports (unit + integration)

- MUST keep imports at the top of the file (never inside functions / never mid-file).
- MUST group imports `std → external → internal` with blank lines.

### Parametrization (unit)

- MUST use `rstest` to consolidate redundant “same test, different input” cases.

## What counts as irrelevant (Rust)

Tests are commonly irrelevant when they:

- Assert trivial pass-through behavior with no branching or risk
- Assert implementation details likely to change without user-visible impact
- Duplicate coverage already present at a more appropriate layer (unit vs integration)
- Add no distinct edge/error case compared to a stronger neighbor test

Keep tests that guard high-risk behaviors:

- Persistence/storage correctness
- Domain validation invariants
- HTTP response contracts (integration tests)

## HTTP integration tests: critical constraints

### Complete response verification is non-negotiable

Do not “dedupe” integration tests by asserting fewer fields.

Per `.cursor/rules/testing-e2e.mdc`:

- For success responses: assert the full JSON shape you consider part of the public contract (including nullables where applicable).
- For error responses: assert status code + stable public error body shape.

### How to reduce duplication without reducing completeness

Allowed tactics:

- Extract a **local helper** that builds the expected JSON `Value` (still enumerating all contract fields).
- Extract a **local helper** that asserts a standard error response shape.
- Use `rstest` when variation is simple and each case still asserts the full shape.

Forbidden:

- Partial/contains JSON matchers that stop enumerating fields
- Replacing explicit JSON assertions with snapshots

## Contract completeness auditor (mandatory procedure)

If the file is under `apps/<app>/tests/*.rs`, run this procedure before deleting or consolidating tests:

1. Identify the endpoint route code under `crates/keydock-http/src/routes/`.
2. Identify the response DTO/type used by the route (and any nested types).
3. Build a checklist of contract fields.
4. Compare checklist vs test assertion; if incomplete, fix completeness first.

## QA / validation (Rust)

Follow `.cursor/rules/rust-qa-loop.mdc`:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test -p <package>
```

When finishing a refinement pass (or when the change has unclear blast radius), run:

```bash
cargo test --workspace
```

## Reporting

When you change tests, report:

- Which tests were removed and why they were irrelevant
- Which tests were consolidated and how (`rstest`, helpers, grouping)
- Confirmation that integration tests still verify the **complete response structure**
- Which rule deviations you detected (assertions/imports/parametrization) and what fixes were applied
- Which validation commands were executed and their outcomes
