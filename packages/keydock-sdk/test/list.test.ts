import { describe, expect, test } from "bun:test";

import { BucketHandle } from "../src/bucket.js";
import { KeydockValidationError } from "../src/errors.js";
import { buildKy } from "../src/internal/http.js";

describe("listing", () => {
  test("listKeys requests JSON keys without values", async () => {
    let url = "";
    const bucket = createBucket((request) => {
      url = request.url;
      return Response.json(["a", "b"]);
    });

    await expect(
      bucket.listKeys({ prefix: "users/", limit: 10, skip: 2, reverse: true }),
    ).resolves.toEqual(["a", "b"]);

    const parsed = new URL(url);
    expect(parsed.pathname).toBe("/api/v1/bucket/");
    expect(parsed.searchParams.get("format")).toBe("json");
    expect(parsed.searchParams.get("values")).toBe("false");
    expect(parsed.searchParams.get("prefix")).toBe("users/");
    expect(parsed.searchParams.get("limit")).toBe("10");
    expect(parsed.searchParams.get("skip")).toBe("2");
    expect(parsed.searchParams.get("reverse")).toBe("true");
  });

  test("listEntries requests JSON tuples with values", async () => {
    let url = "";
    const bucket = createBucket((request) => {
      url = request.url;
      return Response.json([
        ["a", 1],
        ["b", { ok: true }],
      ]);
    });

    await expect(bucket.listEntries({ prefix: "users/" })).resolves.toEqual([
      { key: "a", value: 1 },
      { key: "b", value: { ok: true } },
    ]);

    const parsed = new URL(url);
    expect(parsed.searchParams.get("format")).toBe("json");
    expect(parsed.searchParams.get("values")).toBe("true");
    expect(parsed.searchParams.get("prefix")).toBe("users/");
  });

  test("rejects invalid listing options and response shapes", async () => {
    const invalidOptions = createBucket(() => Response.json([]));
    const invalidShape = createBucket(() => Response.json([["missing-value"]]));

    await expect(invalidOptions.listKeys({ limit: -1 })).rejects.toBeInstanceOf(
      KeydockValidationError,
    );
    await expect(invalidShape.listEntries()).rejects.toBeInstanceOf(KeydockValidationError);
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
