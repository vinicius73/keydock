---
name: plan
description: Planning and execution protocol for multi-phase work in this Rust workspace. Use when the user asks for a plan, phased execution, acceptance criteria, scope control, QA evidence, or validation commands.
---

## Cursor Skill — Planning & Execution Protocol (keydock Rust workspace)

### Why this exists (high-signal only)

This skill prevents generic plans that drift during implementation. It enforces **small phases**, **explicit file-level targets**, **up-front decisions**, and **verifiable acceptance criteria**, with a **hard QA gate before advancing to the next phase**.

### When to use

Use this skill when the user asks for:

- A plan (especially multi-phase)
- Acceptance criteria / scope control / rollout safety
- "What commands should I run?" / QA evidence expectations

---

## Sources of truth (repo-specific)

- `.cursor/rules/project-structure.mdc` (where to implement things; route ↔ test mapping)
- `.cursor/rules/rust-qa-loop.mdc` (format/lint/test commands and minimum tests mapping)
- `.cursor/rules/error-handling.mdc` (typed errors and HTTP error mapping expectations)
- `.cursor/rules/rust-code-style.mdc` + `.cursor/rules/rust-style-imports.mdc`
- `.cursor/rules/rust-style-testing.mdc` + `.cursor/rules/testing-unit.mdc`
- `.cursor/rules/testing-e2e.mdc` (HTTP integration tests; complete response shape assertions)

---

## Repo map (high-signal touchpoints)

This workspace is organized around `apps/` (binaries) and `crates/` (libraries):

- `apps/keydock/src/main.rs`: composition root (wiring, startup)
- `crates/keydock-http/`: Axum routes, DTOs/extractors, OpenAPI, HTTP error mapping
- `crates/keydock-domain/`: pure domain types/validation (no IO)
- `crates/keydock-usecase/`: orchestration + ports (traits)
- `crates/keydock-fjall/`: storage adapter implementation
- `crates/keydock-state/`: shared application state for HTTP layer
- `crates/keydock-config/`: CLI/config load/merge/validate
- `crates/keydock-testkit/`: integration test fixtures (`keydock_testkit::test_app()`)
- HTTP integration tests: `apps/<app>/tests/*.rs`

---

## Plan as an artifact (reviewable, reusable)

- Prefer saving the plan to: `.cursor/plans/<YYYY-MM-DD>-<slug>.md`
- If implementation diverges materially, update the plan (decision log + impacted phases) before continuing.

---

## Non-negotiable QA gates (Rust)

Every phase uses a **closing procedure** (Steps 1–4 below) before it can be marked done.
Step 5 (commit) is **optional** — only execute it when the user explicitly requests `/commit`.

### Phase closing procedure (Steps 1–4 mandatory, Step 5 on request)

**Step 1 — Format + lint + targeted tests**

```bash
just qa <crate(s) touched in this phase>   # fmt + fix + clippy + targeted test
```

`just qa <pkg>` runs `just fix` (fmt + clippy --fix) then `cargo test -p <pkg>`.
Omit the package argument only for the final delivery gate (Step 4).

**Step 2 — `/refine-tests` (mandatory)**

Run the `refine-tests` skill on every new or modified test file in this phase.

Checklist to enforce:

- `rstest` `#[case]` used for parametrized scenarios — no copy-paste test duplication
- `pretty_assertions::assert_eq` everywhere — no bare `assert!` for equality
- HTTP integration tests assert the **complete** response shape (status + full JSON body)
- Imports grouped `std → external → internal` at the top of each file, never inside functions

Each phase plan must list explicitly: **"Files for Step 2 (refine-tests): ..."**

**Step 3 — `/rust-best-practices` (mandatory)**

Delegate all new and modified **production** source files to the `rust-best-practices` subagent.
Must pass with no remaining issues before advancing.

Common checks:

- No `unwrap()`/`expect()` in production paths
- No credential or secret material in `Debug` output or log fields
- No timing-unsafe comparisons (bare `==` on credential/secret bytes)
- All public functions use appropriate visibility (`pub` / `pub(crate)` / private)
- Layer boundaries respected (no HTTP types in domain/usecase, no axum in fjall)
- `?` propagation — no manual `match Ok/Err` when `?` suffices

Each phase plan must list explicitly: **"Files for Step 3 (rust-best-practices): ..."**

**Step 4 — Final QA gate**

```bash
just qa   # fmt + fix + clippy + cargo test --workspace
```

**Step 5 — `/commit` (optional — only when user explicitly requests it)**

Generate a commit message using the `commit` skill and apply it.
Scope: only files changed in this phase.
Do not batch multiple phases into one commit.
Do not run `/commit` proactively — wait for the user to ask.

---

## Plan output requirements (anti-randomness)

- **No vague targets**: always name concrete file paths (typically 2–10 per phase).
- **No undecided plans**: every unknown must be bounded (resolution step + go/no-go).
- **No scope creep**: non-goals must be explicit; refactors only when necessary and bounded.
- **No hidden context**: the plan must stand alone; restate decisions and constraints inside the plan.

---

## Required structure

### Plan header (required; appears once before Phase 1)

- **Context snapshot**: current behavior, desired behavior, definition of done
- **Constraints & invariants** (global): security/privacy, compat, invariants that must not change
- **Decision log** (mandatory): decision, alternatives, rationale, impact
- **Decision → Phase mapping**: which phases implement/validate which decisions
- **Preflight / discovery**:
  - confirmed touchpoints (paths + key symbols)
  - existing coverage (tests already covering this, or "no coverage")
  - contract/source-of-truth (e.g., response DTOs, error shapes, route handlers)
  - bounded unknowns

Preferred decision log format:

| Decision | Chosen | Alternatives | Rationale | Implemented/validated in |
| -------- | ------ | ------------ | --------- | ------------------------ |
| D1: …    | …      | …            | …         | Phase 1, Phase 2         |

### Phase N (template)

- **Objective**: what changes in behavior/outcome
- **Why**: rationale
- **Where**: explicit file paths
- **Scope**: In / Out / Non-goals
- **Decisions**: approach, contract shape, error behavior, tests to add/modify, QA commands
- **Invariants**: what must not change
- **Acceptance criteria**: 2–8 objective checks
- **Security checks** (brief, relevant): authz/tenant boundaries, avoid leaking internal errors, input validation
- **Risks & mitigations**
- **QA plan** (references the closing procedure above):
  - Targeted tests for Step 1: `just qa <package>`
  - Files for Step 2 (refine-tests): explicit list of test files
  - Files for Step 3 (rust-best-practices): explicit list of production source files
  - Step 4 always runs `just qa` (no package — full workspace)

After execution, the phase report MUST include:

- **Self-review notes** (bug hunt, scope audit, decision audit)
- **Refine-tests notes**: summary of what the skill found and fixed, or "no issues"
- **Rust-best-practices notes**: summary of what the subagent found and fixed, or "no issues"
- **QA evidence**: commands executed + pass/fail outcome

### Post-phase report template

```
Phase N — <title>

Self-review notes:
- [ ] No unwrap/expect in production paths
- [ ] No secret/credential material in Debug/log output
- [ ] No timing-unsafe comparisons on credential bytes
- [ ] Visibility correct (pub / pub(crate) / private)
- [ ] Layer boundaries respected

Refine-tests notes:
- Files reviewed: <list>
- Changes made: <none | description>
- rstest applied: <yes/no>
- pretty_assertions: <yes/no>
- Complete response assertions: <yes/no — HTTP tests only>

Rust-best-practices notes:
- Files reviewed: <list>
- Issues found: <none | list>
- Issues fixed: <none | list>

QA evidence:
- just qa <package>: <pass/fail — Step 1>
- just qa: <pass/fail — Step 4, N tests>

Commit: <hash | not requested>
```

---

## Priority clause

If speed conflicts with correctness, correctness wins. If scope is ambiguous, default to the smallest viable scope and record it explicitly.
