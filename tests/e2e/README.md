# Keydock SDK E2E

This suite runs the published SDK package shape against a real `keydock` process.

It is intentionally different from the Rust integration tests and the k6 suite:

- Rust tests validate server behavior in-process through test fixtures.
- k6 validates HTTP contracts and black-box operational flows.
- This suite validates that browser apps can install, bundle, and use `keydock-sdk` through Vite, Vue, Tailwind, and Playwright.

## What Runs

`global-setup.ts` starts one real release `keydock` server on `127.0.0.1:18082` with a temporary data directory. Playwright also starts the Vite app server on `127.0.0.1:5173`.

The mini apps live under `tests/e2e/apps` and import the SDK as:

```ts
import { createKeydock } from "keydock-sdk";
```

They must not import SDK source files by relative path.

## Commands

From the repository root:

```sh
just e2e
```

Run with screenshots and videos retained only for failures:

```sh
just e2e test:artifacts
```

Record screenshots and videos for every test:

```sh
just e2e test:record
```

Open Playwright UI:

```sh
just e2e test:ui
```

UI mode opens the Playwright test runner. It does not run tests automatically:
select a spec and click run. This script uses `--headed --workers=1`, so the
browser window appears while the selected test runs.

Open the last HTML test report:

```sh
just e2e-report
```

From `tests/e2e` directly:

```sh
bun run test
bun run test:artifacts
bun run test:record
bun run test:ui
bun run report
```

## Artifacts

Default runs keep Playwright traces on failure only. Screenshots and videos are opt-in.

Artifacts are written under:

- `tests/e2e/test-results/`
- `tests/e2e/playwright-report/`

Both directories are git-ignored.

## Environment

- `E2E_PORT`: Keydock server port. Defaults to `18082`.
- `KEYDOCK_BIN`: Path to the `keydock` binary. Defaults to `../../target/release/keydock` from `tests/e2e`.
- `E2E_SCREENSHOT`: `off`, `on`, or `only-on-failure`.
- `E2E_VIDEO`: `off`, `on`, `retain-on-failure`, or `on-first-retry`.

Boolean aliases are accepted:

- `E2E_SCREENSHOT=true` means `only-on-failure`.
- `E2E_VIDEO=true` means `retain-on-failure`.

## Security Notes

Tests inject per-test credentials into the browser with `page.addInitScript()`.
Do not pass credentials through URLs, checked-in HTML, or console logs.

Each test uses unique bucket data and cleans up buckets best-effort after assertions.
