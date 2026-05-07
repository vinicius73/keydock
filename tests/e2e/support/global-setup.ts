import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { closeSync, existsSync, mkdtempSync, openSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

import { ensureStateDir, statePath, type ServerState } from "./server-state.js";

const DEFAULT_PORT = 18_082;
const READY_TIMEOUT_MS = 25_000;

export default async function globalSetup(): Promise<void> {
  const port = readPort();
  const baseUrl = `http://127.0.0.1:${port}`;
  const tmpDir = mkdtempSync(join(tmpdir(), "keydock-e2e-"));
  const dataDir = join(tmpDir, "data");
  const logPath = join(tmpDir, "keydock.log");
  const keydockBin = resolve(
    process.cwd(),
    process.env.KEYDOCK_BIN ?? "../../target/release/keydock",
  );

  if (!existsSync(keydockBin)) {
    rmSync(tmpDir, { recursive: true, force: true });
    throw new Error(
      `keydock binary not found at '${keydockBin}'. Run from the repository root: cargo build -p keydock --release`,
    );
  }

  const stdout = openSync(logPath, "a");
  const stderr = openSync(logPath, "a");
  const rootKey = `keydock-e2e-${Date.now()}-${randomUUID()}`;

  const child = spawn(
    keydockBin,
    ["serve", "--listen", `127.0.0.1:${port}`, "--data-dir", dataDir],
    {
      cwd: resolve(process.cwd(), "../.."),
      detached: true,
      env: {
        ...process.env,
        KEYDOCK_ROOT_KEY: rootKey,
        KEYDOCK_RATE_LIMIT_ENABLED: "false",
      },
      stdio: ["ignore", stdout, stderr],
    },
  );

  closeSync(stdout);
  closeSync(stderr);
  child.unref();

  if (child.pid === undefined) {
    throw new Error("failed to start keydock: child process pid is undefined");
  }

  try {
    await waitForReady(`${baseUrl}/ready`, logPath);
  } catch (error) {
    killProcessGroup(child.pid);
    rmSync(tmpDir, { recursive: true, force: true });
    throw error;
  }

  const state: ServerState = {
    baseUrl,
    pid: child.pid,
    tmpDir,
    logPath,
  };

  ensureStateDir();
  writeFileSync(statePath, JSON.stringify(state, null, 2));
  process.env.KEYDOCK_E2E_URL = baseUrl;
  process.env.KEYDOCK_E2E_ROOT_KEY = rootKey;
  process.env.KEYDOCK_E2E_PID = String(child.pid);
  process.env.KEYDOCK_E2E_TMP_DIR = tmpDir;
}

function readPort(): number {
  const raw = process.env.E2E_PORT;
  if (raw === undefined || raw.length === 0) {
    return DEFAULT_PORT;
  }

  const port = Number(raw);
  if (!Number.isInteger(port) || port < 1 || port > 65_535) {
    throw new Error(`E2E_PORT must be an integer between 1 and 65535, got '${raw}'`);
  }
  return port;
}

async function waitForReady(url: string, logPath: string): Promise<void> {
  const deadline = Date.now() + READY_TIMEOUT_MS;
  let lastError: unknown;

  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, { signal: AbortSignal.timeout(1_500) });
      if (response.ok) {
        return;
      }
      lastError = new Error(`readiness probe returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }

    await delay(200);
  }

  throw new Error(
    `keydock did not become ready at ${url} within ${READY_TIMEOUT_MS}ms; log: ${logPath}; last error: ${String(lastError)}`,
  );
}

function killProcessGroup(pid: number): void {
  try {
    process.kill(-pid, "SIGTERM");
  } catch {
    try {
      process.kill(pid, "SIGTERM");
    } catch {
      // Teardown is best-effort because the process may have exited while the readiness probe failed.
    }
  }
}
