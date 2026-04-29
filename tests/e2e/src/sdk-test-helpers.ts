import { createKeydock, KeydockError, KeydockValidationError } from "keydock-sdk";
import type { CreateBucketInput, KeydockClient } from "keydock-sdk";

import type { BucketCredentials } from "./browser-config.js";

type TemporaryBucketInput = CreateBucketInput & { secretKey: string };

export async function captureKeydockError(
  operation: () => Promise<unknown>,
): Promise<KeydockError> {
  try {
    await operation();
  } catch (error) {
    if (error instanceof KeydockError) {
      return error;
    }
    throw error;
  }

  throw new Error("expected operation to fail with KeydockError");
}

export async function captureAnyError(operation: () => Promise<unknown>): Promise<string> {
  try {
    await operation();
  } catch (error) {
    if (error instanceof KeydockError) {
      return `${error.name}:${error.status}`;
    }
    if (error instanceof KeydockValidationError) {
      return error.name;
    }
    throw error;
  }

  throw new Error("expected operation to fail");
}

export function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

export function bucketCreateInput(
  credentials: BucketCredentials,
  options: {
    defaultTtlSeconds?: number;
    fallbackSigningKey?: string;
  } = {},
): CreateBucketInput {
  const input: CreateBucketInput = {
    email: credentials.email,
    secretKey: credentials.secretKey,
    readKey: credentials.readKey,
    writeKey: credentials.writeKey,
  };

  if (options.defaultTtlSeconds !== undefined) {
    input.defaultTtlSeconds = options.defaultTtlSeconds;
  }

  if (credentials.signingKey !== undefined) {
    input.signingKey = credentials.signingKey;
  } else if (options.fallbackSigningKey !== undefined) {
    input.signingKey = options.fallbackSigningKey;
  }

  return input;
}

export function publicBucketSecretKey(credentials: Pick<BucketCredentials, "secretKey">): string {
  return `${credentials.secretKey}-public`;
}

export async function createPublicBucket(
  baseUrl: string,
  credentials: Pick<BucketCredentials, "email" | "secretKey">,
): Promise<string> {
  const client = createKeydock({ baseUrl });
  const created = await client.buckets.create({
    email: `public-${credentials.email}`,
    secretKey: publicBucketSecretKey(credentials),
    defaultTtlSeconds: 0,
  });
  return created.id;
}

export async function withTemporaryBucket(
  baseUrl: string,
  input: TemporaryBucketInput,
  operation: (client: KeydockClient, bucketId: string) => Promise<void>,
): Promise<void> {
  const anonymous = createKeydock({ baseUrl });
  const created = await anonymous.buckets.create(input);
  const client = createKeydock({ baseUrl, auth: input.secretKey });
  try {
    await operation(client, created.id);
  } finally {
    await client.buckets.delete(created.id);
  }
}
