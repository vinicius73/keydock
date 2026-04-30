import { describe, expect, test } from "bun:test";

import { BucketHandle } from "../src/bucket.js";
import { KeydockError, KeydockValidationError } from "../src/errors.js";
import { buildKy, writeRequestOptions } from "../src/internal/http.js";

type RecordedRequest = {
  url: string;
  method: string;
  contentType: string | null;
  body: string;
};

describe("key operations", () => {
  test("get methods use explicit response readers", async () => {
    const bucket = createBucket(async (request) => {
      if (request.url.endsWith("/json")) {
        return Response.json({ name: "Ana" });
      }
      if (request.url.endsWith("/bytes")) {
        return new Response(new Uint8Array([1, 2, 3]));
      }
      return new Response("hello");
    });

    await expect(bucket.getText("text")).resolves.toBe("hello");
    await expect(
      bucket.getJson("json", { parse: (value) => value as { name: string } }),
    ).resolves.toEqual({
      name: "Ana",
    });
    await expect(bucket.getBytes("bytes")).resolves.toEqual(new Uint8Array([1, 2, 3]));
  });

  test("getJsonOrNull distinguishes missing keys from JSON null values", async () => {
    const notFound = createBucket(() => apiErrorResponse(404, "not_found"));
    const jsonNull = createBucket(() => Response.json(null));
    const forbidden = createBucket(() => apiErrorResponse(403, "forbidden"));

    await expect(notFound.getJsonOrNull("missing")).resolves.toBeUndefined();
    await expect(jsonNull.getJsonOrNull("present-null")).resolves.toBeNull();
    await expect(forbidden.getJsonOrNull("secret")).rejects.toBeInstanceOf(KeydockError);
  });

  test("write methods use correct method, path, content type, and ttl", async () => {
    const requests: RecordedRequest[] = [];
    const bucket = createBucket(async (request) => {
      requests.push(await recordRequest(request));
      return new Response("ok");
    });

    await bucket.setText("a/b", "hello", { ttlSeconds: 60 });
    await bucket.setJson("json", { ok: true });
    await bucket.setBytes("bytes", new Uint8Array([65, 66]));

    expect(requests).toEqual([
      {
        url: "https://keydock.example.com/api/v1/bucket/a%2Fb?ttl=60",
        method: "PUT",
        contentType: "text/plain; charset=utf-8",
        body: "hello",
      },
      {
        url: "https://keydock.example.com/api/v1/bucket/json",
        method: "PUT",
        contentType: "application/json",
        body: '{"ok":true}',
      },
      {
        url: "https://keydock.example.com/api/v1/bucket/bytes",
        method: "PUT",
        contentType: "application/octet-stream",
        body: "AB",
      },
    ]);
  });

  test("delete and exists map expected HTTP behavior", async () => {
    const requests: RecordedRequest[] = [];
    const bucket = createBucket(async (request) => {
      requests.push(await recordRequest(request));
      if (request.method === "HEAD" && request.url.endsWith("/missing")) {
        return apiErrorResponse(404, "not_found");
      }
      return new Response(null, {
        status: request.method === "DELETE" ? 204 : 200,
      });
    });

    await expect(bucket.exists("present")).resolves.toBe(true);
    await expect(bucket.exists("missing")).resolves.toBe(false);
    await expect(bucket.delete("present")).resolves.toBeUndefined();

    expect(requests.map((request) => request.method)).toEqual(["HEAD", "HEAD", "DELETE"]);
  });

  test("increment serializes deltas and preserves counter responses", async () => {
    const requests: RecordedRequest[] = [];
    const bucket = createBucket(async (request) => {
      requests.push(await recordRequest(request));
      return new Response(requests.length === 1 ? "42" : "9007199254740993");
    });

    await expect(bucket.increment("views", 1)).resolves.toEqual({
      raw: "42",
      kind: "integer",
      bigint: 42n,
      number: 42,
    });
    await expect(bucket.increment("views", -2n)).resolves.toEqual({
      raw: "9007199254740993",
      kind: "integer",
      bigint: 9007199254740993n,
    });

    expect(requests.map((request) => request.body)).toEqual(["+1", "-2"]);
    expect(requests.every((request) => request.method === "PATCH")).toBe(true);
  });

  test("local validation rejects invalid ttl and counter deltas", async () => {
    const bucket = createBucket(() => new Response("ok"));

    await expect(bucket.setText("k", "v", { ttlSeconds: 1.5 })).rejects.toBeInstanceOf(
      KeydockValidationError,
    );
    await expect(bucket.increment("views", 0)).rejects.toBeInstanceOf(KeydockValidationError);
    await expect(bucket.increment("views", Number.NaN)).rejects.toBeInstanceOf(
      KeydockValidationError,
    );
  });

  test("write request options disable retries", () => {
    expect(writeRequestOptions({ request: { retry: { limit: 5 } } }).retry).toEqual({ limit: 0 });
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

function apiErrorResponse(status: number, message: string): Response {
  return Response.json({ error: { code: status, message } }, { status });
}

async function recordRequest(input: Request): Promise<RecordedRequest> {
  const clone = input.clone();
  return {
    url: input.url,
    method: input.method,
    contentType: input.headers.get("Content-Type"),
    body: await clone.text(),
  };
}
