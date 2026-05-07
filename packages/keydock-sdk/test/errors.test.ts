import { describe, expect, test } from "bun:test";

import {
  KeydockError,
  KeydockNetworkError,
  KeydockTimeoutError,
  KeydockValidationError,
} from "../src/index.js";

describe("SDK errors", () => {
  test("KeydockError exposes stable server error fields", () => {
    const response = new Response("{}", { status: 404 });
    const request = new Request("https://keydock.example.com/api/v1/bucket/key");
    const cause = new Error("upstream");

    const error = new KeydockError({
      status: 404,
      code: 404,
      detail: "not_found",
      response,
      request,
      cause,
    });

    expect(error).toBeInstanceOf(Error);
    expect(error.name).toBe("KeydockError");
    expect(error.message).toBe("Keydock request failed: not_found");
    expect(error.status).toBe(404);
    expect(error.code).toBe(404);
    expect(error.detail).toBe("not_found");
    expect(error.response).toBe(response);
    expect(error.request).toBe(request);
    expect(error.cause).toBe(cause);
  });

  test("network and timeout errors preserve their names", () => {
    expect(new KeydockNetworkError({ message: "network failed" }).name).toBe("KeydockNetworkError");
    expect(new KeydockTimeoutError({ message: "request timed out" }).name).toBe(
      "KeydockTimeoutError",
    );
  });

  test("validation errors are TypeError instances", () => {
    const error = new KeydockValidationError("invalid input");

    expect(error).toBeInstanceOf(TypeError);
    expect(error).toBeInstanceOf(Error);
    expect(error.name).toBe("KeydockValidationError");
  });
});
