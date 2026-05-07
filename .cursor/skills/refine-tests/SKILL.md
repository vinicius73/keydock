---
name: refine-tests
description: Refines Rust tests by removing irrelevant cases, consolidating redundant coverage, and enforcing repo testing rules (pretty_assertions + rstest + complete HTTP response assertions). Use when the user asks to clean up tests, remove redundancy, reduce flakiness/noise, or speed up CI.
---

# Refine Tests (Remove Irrelevant, Consolidate Redundant)

This skill guides how to safely **reduce noise and duplication** in tests while preserving confidence, determinism, and maintainability.

## How to use this skill (expected inputs)

This skill is designed for the workflow: **user provides one or more test files** → you refine those files.

When the user asks to “refine tests”, collect these inputs (preferably as file paths):

- **Target files**: one or more explicit test files (paths)
- **Goal** (optional): what they want improved (speed, readability, dedupe, flakiness)
- **Constraints** (optional): “don’t touch production code”, “only consolidate”, etc.

## Scope control (critical)

Default stance: **focus only on the provided files**.

- Do not do broad refactors.
- Do not “clean up” unrelated tests.
- Only touch non-target files when strictly required (e.g., updating a `mod.rs` registration, or adjusting an existing shared test helper that the target file already uses).
- If you must touch an extra file, keep the diff minimal and explain why it is required for correctness/determinism.

## Scope

Applies to:

- **Rust unit tests** inside crate/app modules: `"{crates,apps}/**/*_test.rs"` or `#[cfg(test)] mod tests { ... }` at the bottom of a `.rs` file.
- **HTTP integration tests**: `apps/<app>/tests/*.rs` (today: mostly under `apps/keydock/tests/`) using `keydock_testkit::TestContext` + `axum_test::TestServer`.

## Non-negotiables (Balanced stance)

- Remove/merge tests only when you can **prove equivalent or better coverage remains**.
- Prefer **consolidation** (parameterization, helpers, grouping) over deletion when unsure.
- Never “simplify” HTTP integration tests by asserting fewer fields. In this repo, **complete response verification is mandatory** (see `.cursor/rules/testing-e2e.mdc`).
- Keep changes small, reviewable, and supported by running the right QA loops.

## Rule compliance is part of refinement (mandatory)

In this repo, “refine tests” also means “bring tests back into compliance with mandatory rules”.

When you touch a Rust test file, you MUST actively look for and fix these deviations:

- **Assertions (Rust)**
  - MUST use `pretty_assertions::assert_eq` for equality assertions.
  - MUST convert `assert!(cond)` style checks into `assert_eq!(cond, true, "...")` to keep diffs readable.
- **Parametrization (Rust)**
  - MUST prefer `rstest` to merge redundant “same test, different input” cases.
- **HTTP integration tests**
  - MUST assert the full JSON shape for the endpoint under test (including nullables when part of the public contract).
  - MUST assert error bodies with the stable public shape (avoid asserting unstable internal/library messages).
- **Imports (Rust)**
  - MUST keep imports at the top of the file (never inside functions).
  - MUST group imports `std → external → internal` with blank lines.

If the user asked to “refine tests” and the target file violates any MUST rule above, the correct action is to fix the violation as part of the refinement pass (even if it increases the diff slightly), because the file is already out of policy.

## Violation severity (how to prioritize)

When multiple opportunities exist, prioritize in this order:

1. **Critical contract violations**: E2E assertions that do not verify the complete response structure.
2. **Test correctness violations**: wrong fixture/app wiring, wrong assumptions about state, flaky timing assumptions.
3. **Mandatory style violations**: `assert!` usage, missing `pretty_assertions`, import placement/grouping.
4. **Refinement opportunities**: redundancy/irrelevance cleanups that do not change semantics.

## Decision tree (remove vs consolidate vs keep)

Use this quick decision tree per test (or group of tests):

1. **Is it the only test covering a behavior?**
   - If yes: **keep**, or consolidate only if the behavior remains fully covered.
2. **Is it testing public behavior (what) vs implementation detail (how)?**
   - If “how”: usually **remove** or rewrite to assert observable behavior.
3. **Is the same behavior already covered at a more appropriate layer?**
   - If yes: prefer keeping **one** high-signal test at the right layer and remove the lower-signal duplicate.
4. **Is it redundant only because of many similar inputs/states?**
   - If yes: **consolidate** into a parameterized/table-driven test.
5. **Does the test meaningfully reduce risk?**
   - High-risk behaviors to preserve: auth, permissions, money/price calculations, persistence, migrations, background jobs, critical E2E flows.

## References (rules and deeper guidance)

This skill is intentionally operational and short. For layer-specific directives and best practices, use:

- `reference-server.md` (Rust unit + HTTP integration)
- `.cursor/rules/rust-style-testing.mdc`
- `.cursor/rules/testing-unit.mdc`
- `.cursor/rules/testing-e2e.mdc`
- `.cursor/rules/rust-qa-loop.mdc`

## Workflow

### 1) Read targets and build a short “test map”

For each provided file:

- Read the whole file.
- List each Rust test (`#[test]`, `#[tokio::test]`, `#[rstest]`) as a row in a small map.
- For each row, write down the **single behavior** it claims to protect.

For each candidate test file, record:

- **Behavior** under test (one sentence, user-facing)
- **Layer** (unit / HTTP integration)
- **Signals asserted** (e.g., return value, error variant, JSON response shape)
- **Runtime / flakiness risk** (slow, timing-sensitive, external dependencies)
- **Redundancy candidates** (same behavior covered elsewhere)

Goal: make deletions/consolidations defensible and reviewable.

### 1.5) E2E Completeness Audit (mandatory for server E2E)

If the target is an HTTP integration test under `apps/<app>/tests/`, audit contract completeness before deleting or consolidating anything:

- Locate the route handler under `crates/keydock-http/src/routes/` and any response DTO/presenter types used.
- Build a checklist of response fields you consider part of the public contract (including nested fields and nullables).
- Compare the checklist with what the test asserts.
- If any field is missing from assertions, fix the test to assert the complete shape first.

Refinement is not allowed to reduce contract coverage.

### 2) Classify tests: keep vs irrelevant vs redundant

For each row in the map, choose exactly one:

- **Keep**: unique, high-signal, correct layer, deterministic.
- **Irrelevant**: low regression value (implementation detail, triviality, library behavior).
- **Redundant**: overlaps a stronger test in the same file or provided set; should be consolidated or removed.

Keep the reasoning short and evidence-based (what signal it asserts, what it duplicates, what risk remains).

### 3) Refine the provided files (minimal diff)

Apply the smallest set of changes that achieves the goal:

- **Remove irrelevant** tests only when you can prove equivalent or better coverage remains.
- **Consolidate redundant** tests by parameterizing or extracting **local** helpers.
- Do not reduce assertion quality to “dedupe”:
  - HTTP integration tests must keep complete response verification (see `reference-server.md` and `.cursor/rules/testing-e2e.mdc`).
- For Rust, ensure you didn’t leave `#[ignore]` unless explicitly intended.

#### Common detectors → fix recipes (Rust)

Use these mechanical checks to catch rule deviations fast:

- **Found `assert!(`**
  - Fix: convert to `assert_eq!(condition, true, "...")` and ensure `use pretty_assertions::assert_eq;` exists.
- **Found multiple tests with same assertion pattern but different inputs**
  - Fix: consolidate into one `#[rstest]` with `#[case::...]` inputs.
- **Found imports inside functions or mid-file**
  - Fix: move imports to the top of file and group `std → external → internal`.
- **E2E tests asserting only a subset of fields**
  - Fix: do a completeness audit against the response DTO/contract and expand the asserted JSON literal to include ALL contract fields (including `null` where applicable).

### 4) Validate (mandatory)

Run the appropriate QA loop for the layer you touched and do not skip failing steps:

- Rust validation: see `.cursor/rules/rust-qa-loop.mdc` (always run `cargo fmt --all`, `cargo clippy --workspace --all-targets`, and the relevant `cargo test -p <package>`; run `cargo test --workspace` when finishing).

## Output expectations (when reporting results)

When you finish a refinement pass, summarize:

- **Removed tests**: what they covered, why irrelevant, and what remains covering the behavior
- **Consolidated tests**: what was merged (and how: `rstest`, case tables, helpers)
- **Net effect**: fewer files/tests, lower runtime, less noise (if applicable)
- **Rule compliance fixes**: which MUST-rule deviations you detected and how you fixed them
- **Evidence**: which QA commands were run and their outcomes

Recommended report format:

```text
Targets:
- <file1>
- <file2>

Changes:
- Removed: <test name> — <reason> — coverage remains in <test name>
- Consolidated: <tests> → <new structure> (rstest/test.each/helper)

Validation:
- cargo fmt --all
- cargo clippy --workspace --all-targets
- cargo test -p <package>
- cargo test --workspace (when finishing)
```
