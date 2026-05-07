import { describe, expect, test } from "bun:test";

import { KeydockValidationError } from "../../src/errors.js";
import { encodeBucketId, encodeKey, normalizeBaseUrl } from "../../src/internal/encoding.js";

describe("encoding", () => {
  test.each([
    ["https://host", "https://host/api/v1/"],
    ["https://host/", "https://host/api/v1/"],
    ["https://host/api/v1", "https://host/api/v1/"],
    ["https://host/api/v1/", "https://host/api/v1/"],
  ])("normalizes %s", (input, expected) => {
    expect(normalizeBaseUrl(input)).toBe(expected);
  });

  test.each([
    ["a/b", "a%2Fb"],
    ["a b", "a%20b"],
    ["a%20b", "a%2520b"],
    ["olá", "ol%C3%A1"],
  ])("encodes key %s exactly once", (input, expected) => {
    expect(encodeKey(input)).toBe(expected);
  });

  test("encodes bucket ids defensively", () => {
    expect(encodeBucketId("bucket/id")).toBe("bucket%2Fid");
  });

  test("rejects empty path segments", () => {
    expect(() => encodeKey("")).toThrow(KeydockValidationError);
    expect(() => encodeBucketId("")).toThrow(KeydockValidationError);
  });
});
