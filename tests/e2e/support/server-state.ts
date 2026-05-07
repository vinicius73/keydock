import { mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

export type ServerState = {
  baseUrl: string;
  pid: number;
  tmpDir: string;
  logPath: string;
};

export const statePath = resolve(import.meta.dirname, "../.playwright/keydock-state.json");

export function ensureStateDir(): void {
  mkdirSync(dirname(statePath), { recursive: true });
}
