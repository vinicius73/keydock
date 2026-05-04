import { describe, expect, test } from "bun:test";
import { HTTPError, TimeoutError, type NormalizedOptions } from "ky";

import {
  KeydockError,
  KeydockNetworkError,
  KeydockTimeoutError,
  KeydockValidationError,
} from "../../src/errors.js";
import {
  isNotFoundError,
  normalizeKyError,
  parseCounterResponse,
  parseErrorBody,
} from "../../src/internal/response.js";

const emptyKyOptions = {} as NormalizedOptions;

describe("response", () => {
  test.each([
    [
      "42",
      {
        raw: "42",
        kind: "integer",
        bigint: 42n,
        number: 42,
      },
    ],
    [
      "9007199254740993",
      {
        raw: "9007199254740993",
        kind: "integer",
        bigint: 9007199254740993n,
      },
    ],
    [
      "1.5",
      {
        raw: "1.5",
        kind: "float",
        number: 1.5,
      },
    ],
  ])("parseCounterResponse parses %s", (input, expected) => {
    expect(parseCounterResponse(input)).toEqual(expected);
  });

  test("rejects invalid counter payloads", () => {
    expect(() => parseCounterResponse("not-a-number")).toThrow(KeydockValidationError);
  });

  test("normalizes Keydock error envelopes", async () => {
    const response = Response.json(
      {
        error: {
          code: 404,
          message: "not_found",
        },
      },
      { status: 404 },
    );
    const request = new Request("https://keydock.example.com/api/v1/bucket/key");
    const cause = new Error("http");

    const error = await parseErrorBody(response, request, cause);

    expect(error).toBeInstanceOf(KeydockError);
    expect(error.status).toBe(404);
    expect(error.code).toBe(404);
    expect(error.detail).toBe("not_found");
    expect(error.request).toBe(request);
    expect(error.cause).toBe(cause);
  });

  test("falls back to statusText when body is not JSON", async () => {
    const response = new Response("<html>error</html>", {
      status: 502,
      statusText: "Bad Gateway",
    });

    const error = await parseErrorBody(response, undefined, undefined);

    expect(error).toBeInstanceOf(KeydockError);
    expect(error.status).toBe(502);
    expect(error.code).toBe(502);
    expect(error.detail).toBe("Bad Gateway");
  });

  test("falls back to request_failed when statusText is empty and body is not JSON", async () => {
    const response = new Response("nope", { status: 500, statusText: "" });

    const error = await parseErrorBody(response, undefined, undefined);

    expect(error.code).toBe(500);
    expect(error.detail).toBe("request_failed");
  });

  test("uses numeric envelope code when present", async () => {
    const response = Response.json(
      { error: { code: 123, message: "custom" } },
      { status: 400, statusText: "Bad Request" },
    );

    const error = await parseErrorBody(response, undefined, undefined);

    expect(error.code).toBe(123);
    expect(error.detail).toBe("custom");
  });

  test("normalizeKyError maps HTTPError through parseErrorBody", async () => {
    const response = new Response("plain", { status: 400, statusText: "Bad Request" });
    const request = new Request("https://keydock.example.com/x");
    const httpError = new HTTPError(response, request, emptyKyOptions);

    await expect(normalizeKyError(httpError)).rejects.toMatchObject({
      name: "KeydockError",
      status: 400,
      code: 400,
      detail: "Bad Request",
    });
  });

  test("normalizeKyError maps TimeoutError", async () => {
    const request = new Request("https://keydock.example.com/x");
    const timeoutError = new TimeoutError(request);

    await expect(normalizeKyError(timeoutError)).rejects.toBeInstanceOf(KeydockTimeoutError);
  });

  test("normalizeKyError maps unknown errors to KeydockNetworkError", async () => {
    await expect(normalizeKyError(new TypeError("offline"))).rejects.toBeInstanceOf(
      KeydockNetworkError,
    );
  });

  test.each([
    [
      "HTTPError 404",
      () =>
        new HTTPError(
          new Response(null, { status: 404 }),
          new Request("https://keydock.example.com/missing"),
          emptyKyOptions,
        ),
      true,
    ] as const,
    [
      "HTTPError 403",
      () =>
        new HTTPError(
          new Response(null, { status: 403 }),
          new Request("https://keydock.example.com/x"),
          emptyKyOptions,
        ),
      false,
    ] as const,
    [
      "KeydockError 404",
      () =>
        new KeydockError({
          status: 404,
          code: 404,
          detail: "not_found",
        }),
      true,
    ] as const,
    [
      "KeydockError 400",
      () =>
        new KeydockError({
          status: 400,
          code: 400,
          detail: "bad_request",
        }),
      false,
    ] as const,
  ])("isNotFoundError: %s", (_label, factory, expected) => {
    expect(isNotFoundError(factory())).toBe(expected);
  });
});
