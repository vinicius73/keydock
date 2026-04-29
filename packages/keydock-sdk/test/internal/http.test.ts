import { describe, expect, test } from "bun:test";

import { buildKy, writeRequestOptions } from "../../src/internal/http.js";

type RecordedRequest = {
  url: string;
  method: string;
  authorization: string | null;
};

describe("http", () => {
  test("injects bearer auth and normalizes the API prefix", async () => {
    const requests: RecordedRequest[] = [];
    const http = buildKy({
      baseUrl: "https://keydock.example.com",
      auth: "secret",
      request: {
        fetch: async (request) => {
          requests.push(recordRequest(request));
          return new Response("ok");
        },
      },
    });

    await http.get("bucket/key").text();

    expect(requests).toEqual([
      {
        url: "https://keydock.example.com/api/v1/bucket/key",
        method: "GET",
        authorization: "Bearer secret",
      },
    ]);
  });

  test("omits authorization for anonymous requests", async () => {
    let authorization: string | null = "unexpected";
    const http = buildKy({
      baseUrl: "https://keydock.example.com",
      request: {
        fetch: async (request) => {
          authorization = new Request(request).headers.get("Authorization");
          return new Response("ok");
        },
      },
    });

    await http.get("bucket/key").text();

    expect(authorization).toBeNull();
  });

  test("evaluates auth providers before each request", async () => {
    let calls = 0;
    const credentials: string[] = [];
    const http = buildKy({
      baseUrl: "https://keydock.example.com",
      auth: () => {
        calls += 1;
        return `token-${calls}`;
      },
      request: {
        fetch: async (request) => {
          credentials.push(new Request(request).headers.get("Authorization") ?? "");
          return new Response("ok");
        },
      },
    });

    await http.get("bucket/one").text();
    await http.get("bucket/two").text();

    expect(credentials).toEqual(["Bearer token-1", "Bearer token-2"]);
  });

  test("write request options disable retries", () => {
    expect(writeRequestOptions({ request: { retry: { limit: 3 } } }).retry).toEqual({ limit: 0 });
  });
});

function recordRequest(input: RequestInfo | URL): RecordedRequest {
  const request = new Request(input);
  return {
    url: request.url,
    method: request.method,
    authorization: request.headers.get("Authorization"),
  };
}
