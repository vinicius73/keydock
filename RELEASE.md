# Release process

This repository ships the **keydock** server (Rust), a **Docker** image on GitHub Container Registry (GHCR), and the **TypeScript SDK** to **npm**, **GitHub Packages**, and **JSR**. Releases are automated with GitHub Actions after you push an annotated semver **git tag**.

## Overview

1. **Prepare**: Run workflow [Release (prepare)](.github/workflows/release-prepare.yml) manually with the target version. It opens a PR that bumps all version fields and refreshes `Cargo.lock`.
2. **Review & merge**: Ensure CI is green, then merge the PR into `master`.
3. **Tag**: Create and push `v<VERSION>` from `master` (only after merge.)
4. **Publish**: Pushing the tag runs [Release (publish)](.github/workflows/release-publish.yml), which builds and publishes every artifact, then creates a **GitHub Release** with the Linux binary and checksum.

Version **must stay in sync** across:

- Root [`Cargo.toml`](Cargo.toml) — `[workspace.package]` `version`
- [`packages/keydock-sdk/package.json`](packages/keydock-sdk/package.json) — `version`
- [`packages/keydock-sdk/jsr.json`](packages/keydock-sdk/jsr.json) — `version`

The publish workflow refuses to run if the tag (without the `v` prefix) does not match all three.

## Prerequisites

| Requirement | Purpose |
|-------------|---------|
| Repository secret **`NPM_TOKEN`** | Automation token with publish rights for the public npm package `keydock-sdk`. Configure under **Settings → Secrets and variables → Actions**. |
| **JSR** | The `@keydock/sdk` scope must be set up on [jsr.io](https://jsr.io) and linked to this GitHub repository so `bunx jsr publish` can use OIDC in Actions. |
| **GitHub Packages** | Publishing `@vinicius73/keydock-sdk` uses the workflow `GITHUB_TOKEN` with `packages: write` on the relevant jobs (no extra secret). |
| **GHCR** | Docker push uses `GITHUB_TOKEN` with `packages: write`. |

If `NPM_TOKEN` is missing or invalid, the **Release (publish)** run fails at `publish-npm`. Docker, JSR, or GitHub Packages jobs may have succeeded earlier; treat a failed run as **not a complete release** until you fix the failure and either re-run the workflow or address partial publishes manually.

## Step 1 — Prepare (version bump PR)

1. In GitHub: **Actions → Release (prepare) → Run workflow**.
2. Input **version** as semver: `MAJOR.MINOR.PATCH` with optional pre-release, e.g. `0.3.0` or `1.0.0-rc.1`.
3. The workflow will:
   - Reject invalid semver or if `refs/tags/v<VERSION>` already exists on `origin`.
   - Bump `Cargo.toml`, `package.json`, `jsr.json`, and refresh **`Cargo.lock`** via `cargo metadata` (required for `--locked` CI).
   - Push branch `release/v<VERSION>` and open a PR targeting **`master`**.

Workflow file: [.github/workflows/release-prepare.yml](.github/workflows/release-prepare.yml).

## Step 2 — Merge

Resolve review items, confirm the checklist in the PR body, and merge into **`master`** when CI passes (`CI (Rust)`, `CI (SDK)`, etc., as triggered by path filters).

## Step 3 — Tag (start publish)

From an up-to-date `master`:

```bash
git checkout master
git pull
git tag v<VERSION>
git push origin v<VERSION>
```

Replace `<VERSION>` with the same value you used in the prepare workflow (no leading `v` in the manifests—only on the git tag).

**Tag pattern:** `v` followed by semver; pre-releases are allowed (e.g. `v1.0.0-beta.1`). This must match the [Release (publish)](.github/workflows/release-publish.yml) `on.push.tags` filter.

## What Release (publish) does

After the tag push, jobs run with **`cancel-in-progress: false`** so an in-flight release is not cancelled accidentally.

| Job | Output |
|-----|--------|
| **validate-tag** | Ensures tag (no `v`) equals versions in `Cargo.toml`, `package.json`, and `jsr.json`. |
| **build-docker** | Builds and pushes **`ghcr.io/<owner>/keydock`** for `linux/amd64`, with image tags derived from the semver tag and `latest=auto` (stable releases only). Includes provenance and SBOM. |
| **build-binary** | Builds static **`x86_64-unknown-linux-musl`** binary `keydock`, ships as `keydock-linux-amd64` + `.sha256`, and attaches a build provenance attestation. |
| **build-sdk** | Runs `bun run build` in `packages/keydock-sdk` and uploads `dist/` as an artifact. |
| **publish-npm** | Publishes **`keydock-sdk`** to the public npm registry with `npm publish --provenance` (requires **`NPM_TOKEN`**). |
| **publish-ghp** | Temporarily sets package name to **`@vinicius73/keydock-sdk`** and publishes to **GitHub Packages** (npm registry). |
| **publish-jsr** | Publishes **`@keydock/sdk`** from [`jsr.json`](packages/keydock-sdk/jsr.json) via `bunx jsr publish` (source export; OIDC). |
| **create-github-release** | Runs **only after** all of the above succeed. Creates a GitHub Release for the tag with generated notes and attaches **`keydock-linux-amd64`** and **`keydock-linux-amd64.sha256`**. |

Workflow file: [.github/workflows/release-publish.yml](.github/workflows/release-publish.yml).

## Package names by registry

Registry constraints require **different published names** for the SDK:

| Registry | Package name |
|----------|----------------|
| npm (public) | `keydock-sdk` |
| GitHub Packages (npm) | `@vinicius73/keydock-sdk` |
| JSR | `@keydock/sdk` |

The repository’s committed [`package.json`](packages/keydock-sdk/package.json) keeps the **npm public** name and version; the publish workflow patches the name only on the runner for GitHub Packages.

## Local checks (optional)

Before tagging, you can approximate CI locally:

- Rust: `just qa`
- SDK: `just sdk-qa`

## Support files

- Bun install cache and frozen lockfile setup: [.github/actions/bun-setup/action.yml](.github/actions/bun-setup/action.yml)
- Rust toolchain and Cargo cache: [.github/actions/rust-setup/action.yml](.github/actions/rust-setup/action.yml)
