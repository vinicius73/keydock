import { existsSync, readFileSync } from "node:fs";

import { createKeydock, type KeydockClient, type TokenPermission } from "keydock-sdk";

import { statePath, type ServerState } from "./server-state.js";
import type { BucketData } from "./test-data.js";

export type CreatedBucketFixture = {
  id: string;
  credentials: BucketData;
};

export function e2eBaseUrl(): string {
  if (process.env.KEYDOCK_E2E_URL !== undefined && process.env.KEYDOCK_E2E_URL.length > 0) {
    return process.env.KEYDOCK_E2E_URL;
  }

  if (!existsSync(statePath)) {
    throw new Error("KEYDOCK_E2E_URL is not set and keydock state file does not exist");
  }

  const state = JSON.parse(readFileSync(statePath, "utf8")) as ServerState;
  return state.baseUrl;
}

export function createClient(auth?: string): KeydockClient {
  const options = {
    baseUrl: e2eBaseUrl(),
  };

  return auth === undefined ? createKeydock(options) : createKeydock({ ...options, auth });
}

export async function createBucket(credentials: BucketData): Promise<CreatedBucketFixture> {
  const client = createClient();
  const created = await client.buckets.create({
    email: credentials.email,
    secretKey: credentials.secretKey,
    readKey: credentials.readKey,
    writeKey: credentials.writeKey,
    signingKey: credentials.signingKey,
    defaultTtlSeconds: 0,
  });

  return {
    id: created.id,
    credentials,
  };
}

export async function createScopedToken(
  bucketId: string,
  secretKey: string,
  input: {
    prefix: string;
    permissions: TokenPermission[];
    ttlSeconds?: number;
  },
): Promise<string> {
  const client = createClient(secretKey);
  const token = await client.bucket(bucketId).tokens.create({
    prefix: input.prefix,
    permissions: input.permissions,
    ttlSeconds: input.ttlSeconds ?? 900,
  });
  return token.accessToken;
}

export async function deleteBucketBestEffort(
  fixture: CreatedBucketFixture | undefined,
): Promise<void> {
  if (fixture === undefined) {
    return;
  }

  try {
    await createClient(fixture.credentials.secretKey).buckets.delete(fixture.id);
  } catch {
    // Cleanup must not hide the assertion that caused the test to fail.
  }
}
