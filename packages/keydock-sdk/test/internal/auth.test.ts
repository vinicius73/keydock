import { describe, expect, test } from "bun:test";

import { resolveAuth } from "../../src/internal/auth.js";

describe("auth", () => {
  test("resolves missing auth to anonymous requests", async () => {
    await expect(resolveAuth(undefined)).resolves.toBeUndefined();
  });

  test("resolves static credentials", async () => {
    await expect(resolveAuth("secret")).resolves.toBe("secret");
  });

  test("calls async auth providers for each request", async () => {
    let calls = 0;
    const auth = async () => {
      calls += 1;
      return `token-${calls}`;
    };

    await expect(resolveAuth(auth)).resolves.toBe("token-1");
    await expect(resolveAuth(auth)).resolves.toBe("token-2");
    expect(calls).toBe(2);
  });
});
