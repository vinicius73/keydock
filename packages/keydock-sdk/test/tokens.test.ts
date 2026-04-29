import { describe, expect, test } from "bun:test";

import { BucketHandle } from "../src/bucket.js";
import { KeydockValidationError } from "../src/errors.js";
import { buildKy } from "../src/internal/http.js";

type RecordedRequest = {
  url: string;
  method: string;
  contentType: string | null;
  body: string;
};

describe("tokens", () => {
  test("creates scoped tokens with form encoding", async () => {
    let recorded: RecordedRequest | undefined;
    const bucket = createBucket(async (request) => {
      recorded = await recordRequest(request);
      return Response.json({ access_token: "token-value" });
    });

    await expect(
      bucket.tokens.create({
        prefix: "public/",
        permissions: ["read", "enumerate"],
        ttlSeconds: 900,
      }),
    ).resolves.toEqual({ accessToken: "token-value" });

    expect(recorded?.url).toBe("https://keydock.example.com/api/v1/bucket/tokens/");
    expect(recorded?.method).toBe("POST");
    expect(recorded?.contentType).toBe("application/x-www-form-urlencoded");
    expect(new URLSearchParams(recorded?.body)).toEqual(
      new URLSearchParams({
        prefix: "public/",
        permissions: "read,enumerate",
        ttl: "900",
      }),
    );
  });

  test("rejects invalid token inputs before sending", async () => {
    let calls = 0;
    const bucket = createBucket(() => {
      calls += 1;
      return Response.json({ access_token: "token-value" });
    });

    await expect(
      bucket.tokens.create({ prefix: "", permissions: ["read"], ttlSeconds: 900 }),
    ).rejects.toBeInstanceOf(KeydockValidationError);
    await expect(
      bucket.tokens.create({ prefix: "public/", permissions: [], ttlSeconds: 900 }),
    ).rejects.toBeInstanceOf(KeydockValidationError);
    await expect(
      bucket.tokens.create({ prefix: "public/", permissions: ["read"], ttlSeconds: 0 }),
    ).rejects.toBeInstanceOf(KeydockValidationError);
    await expect(
      bucket.tokens.create({
        prefix: "public/",
        permissions: ["read", "read"],
        ttlSeconds: 900,
      }),
    ).rejects.toBeInstanceOf(KeydockValidationError);
    expect(calls).toBe(0);
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
    contentType: normalizeContentType(input.headers.get("Content-Type")),
    body: await clone.text(),
  };
}

function normalizeContentType(contentType: string | null): string | null {
  return contentType?.split(";")[0] ?? null;
}
