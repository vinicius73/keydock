import { describe, expect, test } from "bun:test";

import { KeydockError, KeydockValidationError } from "../../src/errors.js";
import { parseCounterResponse, parseErrorBody } from "../../src/internal/response.js";

describe("response", () => {
  test("parses safe integer counter values", () => {
    expect(parseCounterResponse("42")).toEqual({
      raw: "42",
      kind: "integer",
      bigint: 42n,
      number: 42,
    });
  });

  test("parses large integer counter values without precision loss", () => {
    expect(parseCounterResponse("9007199254740993")).toEqual({
      raw: "9007199254740993",
      kind: "integer",
      bigint: 9007199254740993n,
    });
  });

  test("parses float counter values", () => {
    expect(parseCounterResponse("1.5")).toEqual({
      raw: "1.5",
      kind: "float",
      number: 1.5,
    });
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
});
