import { describe, expect, test } from "bun:test";

import { BucketHandle } from "../src/bucket.js";
import { KeydockValidationError } from "../src/errors.js";
import { buildKy } from "../src/internal/http.js";
import { serializeTransaction } from "../src/transactions.js";

type RecordedRequest = {
  url: string;
  method: string;
  contentType: string | null;
  body: unknown;
};

describe("transactions", () => {
  test("serializes set and delete operations for the server contract", async () => {
    let recorded: RecordedRequest | undefined;
    const bucket = createBucket(async (request) => {
      recorded = await recordRequest(request);
      return new Response(null, { status: 204 });
    });

    await bucket.transaction([
      { set: "users/42/name", value: "Ana", ttlSeconds: 60 },
      { delete: "users/42/tmp" },
    ]);

    expect(recorded).toEqual({
      url: "https://keydock.example.com/api/v1/bucket",
      method: "POST",
      contentType: "application/json",
      body: {
        txn: [{ set: "users%2F42%2Fname", value: "Ana", ttl: 60 }, { delete: "users%2F42%2Ftmp" }],
      },
    });
  });

  test("rejects invalid transaction inputs before sending requests", async () => {
    let calls = 0;
    const bucket = createBucket(() => {
      calls += 1;
      return new Response(null, { status: 204 });
    });

    await expect(bucket.transaction([])).rejects.toBeInstanceOf(KeydockValidationError);
    await expect(bucket.transaction([{ set: "key", value: null }])).rejects.toBeInstanceOf(
      KeydockValidationError,
    );
    await expect(
      bucket.transaction([{ set: "key", value: "value", ttlSeconds: -1 }]),
    ).rejects.toBeInstanceOf(KeydockValidationError);
    expect(calls).toBe(0);
  });

  test("uses the same key encoding rule as path operations", () => {
    expect(serializeTransaction([{ set: "a/b %", value: true }])).toEqual([
      { set: "a%2Fb%20%25", value: true },
    ]);
  });
});

function createBucket(fetch: (request: Request) => Response | Promise<Response>): BucketHandle {
  const http = buildKy({
    baseUrl: "https://keydock.example.com",
    request: {
      fetch: async (input) => fetch(new Request(input)),
    },
  });
  return new BucketHandle("bucket", http);
}

async function recordRequest(input: Request): Promise<RecordedRequest> {
  const clone = input.clone();
  return {
    url: input.url,
    method: input.method,
    contentType: input.headers.get("Content-Type"),
    body: await clone.json(),
  };
}
