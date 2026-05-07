import { describe, expect, test } from "bun:test";

import { BucketsNamespace } from "../src/buckets.js";
import { KeydockValidationError } from "../src/errors.js";
import { buildKy } from "../src/internal/http.js";

type RecordedRequest = {
  url: string;
  method: string;
  contentType: string | null;
  body: string;
};

describe("bucket administration", () => {
  test("create sends form data and wraps the text response", async () => {
    let recorded: RecordedRequest | undefined;
    const buckets = createBuckets(async (request) => {
      recorded = await recordRequest(request);
      return new Response("bucket-id", {
        headers: { "Content-Type": "text/plain; charset=utf-8" },
      });
    });

    await expect(
      buckets.create({
        email: "admin@example.com",
        secretKey: "secret",
        readKey: "read",
        writeKey: "write",
        signingKey: "signing",
        defaultTtlSeconds: 604800,
      }),
    ).resolves.toEqual({ id: "bucket-id" });

    expect(recorded?.url).toBe("https://keydock.example.com/api/v1/");
    expect(recorded?.method).toBe("POST");
    expect(recorded?.contentType).toBe("application/x-www-form-urlencoded");
    expect(new URLSearchParams(recorded?.body)).toEqual(
      new URLSearchParams({
        email: "admin@example.com",
        secret_key: "secret",
        read_key: "read",
        write_key: "write",
        signing_key: "signing",
        default_ttl: "604800",
      }),
    );
  });

  test("getPolicy converts snake_case response fields to camelCase", async () => {
    const buckets = createBuckets(() =>
      Response.json({
        default_ttl: 60,
        has_secret_key: true,
        has_read_key: false,
        has_write_key: true,
        has_signing_key: true,
        signing_key_generation: 3,
        anonymous_access: {
          read: true,
          write: false,
          enumerate: false,
          delete: false,
        },
      }),
    );

    await expect(buckets.getPolicy("bucket")).resolves.toEqual({
      defaultTtlSeconds: 60,
      hasSecretKey: true,
      hasReadKey: false,
      hasWriteKey: true,
      hasSigningKey: true,
      signingKeyGeneration: 3,
      anonymousAccess: {
        read: true,
        write: false,
        enumerate: false,
        delete: false,
      },
    });
  });

  test("updatePolicy sends only explicit snake_case fields", async () => {
    let recorded: RecordedRequest | undefined;
    const buckets = createBuckets(async (request) => {
      recorded = await recordRequest(request);
      return new Response(null, { status: 204 });
    });

    await buckets.updatePolicy("bucket", {
      readKey: "new-read",
      writeKey: null,
      defaultTtlSeconds: 0,
    });

    expect(recorded).toEqual({
      url: "https://keydock.example.com/api/v1/bucket",
      method: "PATCH",
      contentType: "application/json",
      body: '{"read_key":"new-read","write_key":null,"default_ttl":0}',
    });
  });

  test("updatePolicy rejects secretKey null before sending", async () => {
    let calls = 0;
    const buckets = createBuckets(() => {
      calls += 1;
      return new Response(null, { status: 204 });
    });

    await expect(buckets.updatePolicy("bucket", { secretKey: null })).rejects.toBeInstanceOf(
      KeydockValidationError,
    );
    expect(calls).toBe(0);
  });

  test("delete and exists map expected HTTP behavior", async () => {
    const requests: RecordedRequest[] = [];
    const buckets = createBuckets(async (request) => {
      requests.push(await recordRequest(request));
      if (request.method === "HEAD" && request.url.endsWith("/missing")) {
        return Response.json({ error: { code: 404, message: "not_found" } }, { status: 404 });
      }

      return new Response(null, {
        status: request.method === "DELETE" ? 204 : 200,
      });
    });

    await expect(buckets.exists("bucket")).resolves.toBe(true);
    await expect(buckets.exists("missing")).resolves.toBe(false);
    await expect(buckets.delete("bucket")).resolves.toBeUndefined();

    expect(requests.map((request) => [request.method, new URL(request.url).pathname])).toEqual([
      ["HEAD", "/api/v1/bucket"],
      ["HEAD", "/api/v1/missing"],
      ["DELETE", "/api/v1/bucket/"],
    ]);
  });
});

function createBuckets(
  fetch: (request: Request) => Response | Promise<Response>,
): BucketsNamespace {
  return new BucketsNamespace(
    buildKy({
      baseUrl: "https://keydock.example.com",
      request: {
        fetch: async (input) => fetch(new Request(input)),
      },
    }),
  );
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
