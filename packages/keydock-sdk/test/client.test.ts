import { describe, expect, test } from "bun:test";
import ky from "ky";

import { createKeydock } from "../src/index.js";

describe("client", () => {
  test("creates bucket handles that share the configured HTTP client", async () => {
    const requests: string[] = [];
    const keydock = createKeydock({
      baseUrl: "https://keydock.example.com",
      auth: "secret",
      request: {
        fetch: async (input) => {
          const request = new Request(input);
          requests.push(`${request.method} ${request.url} ${request.headers.get("Authorization")}`);
          return new Response("hello");
        },
      },
    });

    await expect(keydock.bucket("bucket").getText("greeting")).resolves.toBe("hello");

    expect(requests).toEqual([
      "GET https://keydock.example.com/api/v1/bucket/greeting Bearer secret",
    ]);
  });

  test("exposes bucket administration namespace", async () => {
    const keydock = createKeydock({
      baseUrl: "https://keydock.example.com",
      request: {
        fetch: async () => new Response(null, { status: 200 }),
      },
    });

    await expect(keydock.buckets.exists("bucket")).resolves.toBe(true);
  });

  test("uses custom Ky instances", async () => {
    const requests: string[] = [];
    const http = ky.create({
      fetch: async (input) => {
        const request = new Request(input);
        requests.push(request.url);
        return new Response("custom");
      },
    });

    const keydock = createKeydock({
      baseUrl: "https://keydock.example.com",
      http,
    });

    await expect(keydock.bucket("bucket").getText("key")).resolves.toBe("custom");
    expect(requests).toEqual(["https://keydock.example.com/api/v1/bucket/key"]);
  });
});
