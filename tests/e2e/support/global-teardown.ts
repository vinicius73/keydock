import { existsSync, readFileSync, rmSync } from "node:fs";
import { setTimeout as delay } from "node:timers/promises";

import { statePath, type ServerState } from "./server-state.js";

export default async function globalTeardown(): Promise<void> {
  const state = readState();
  if (state === undefined) {
    return;
  }

  await stopKeydock(state.pid);
  rmSync(state.tmpDir, { recursive: true, force: true });
  rmSync(statePath, { force: true });
}

function readState(): ServerState | undefined {
  if (!existsSync(statePath)) {
    return undefined;
  }

  return JSON.parse(readFileSync(statePath, "utf8")) as ServerState;
}

async function stopKeydock(pid: number): Promise<void> {
  signal(pid, "SIGTERM");

  for (let attempt = 0; attempt < 50; attempt += 1) {
    if (!isRunning(pid)) {
      return;
    }
    await delay(100);
  }

  signal(pid, "SIGKILL");
}

function isRunning(pid: number): boolean {
  try {
    process.kill(-pid, 0);
    return true;
  } catch {
    try {
      process.kill(pid, 0);
      return true;
    } catch {
      return false;
    }
  }
}

function signal(pid: number, signalName: NodeJS.Signals): void {
  try {
    process.kill(-pid, signalName);
  } catch {
    try {
      process.kill(pid, signalName);
    } catch {
      // The server may already be gone by the time teardown runs.
    }
  }
}
