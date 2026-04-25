import { parseJson } from "./client.js";
import { expect } from "./testing.js";

export function assertJsonEquals(actual, expected, ctx) {
  expect(actual, ctx).toEqual(expected);
}

export function assertContentType(res, expected, ctx) {
  expect(res.headers["Content-Type"], `${ctx}: Content-Type`).toBe(expected);
}

export function assertEmptyBody(res, ctx) {
  const body = res.body;
  if (body === undefined || body === null) return;
  if (typeof body === "string" && body.length === 0) return;
  if (body && body.byteLength === 0) return;
  throw new Error(`${ctx}: expected empty body`);
}

export function assertNoSensitiveFields(jsonBody, fields, ctx) {
  for (const f of fields) {
    expect(
      jsonBody && Object.prototype.hasOwnProperty.call(jsonBody, f),
      `${ctx}: must not expose '${f}'`,
    ).toBeFalsy();
  }
}

export function assertBinaryEquals(res, expectedBytes, ctx) {
  const body = res.body;
  if (!body || body.byteLength === undefined) {
    throw new Error(`${ctx}: expected binary response body`);
  }
  const got = new Uint8Array(body);
  const expected =
    expectedBytes instanceof Uint8Array
      ? expectedBytes
      : new Uint8Array(expectedBytes);
  if (got.length !== expected.length) {
    throw new Error(
      `${ctx}: binary length mismatch expected=${expected.length} got=${got.length}`,
    );
  }
  for (let i = 0; i < got.length; i += 1) {
    if (got[i] !== expected[i]) {
      throw new Error(`${ctx}: binary mismatch at offset ${i}`);
    }
  }
}

export function assertApiError(res, expectedCode, expectedMessage, ctx) {
  expect(res.status, `${ctx}: status`).toBe(expectedCode);
  expect(res.headers["Content-Type"], `${ctx}: Content-Type`).toBe(
    "application/json",
  );
  const body = parseJson(res, ctx);
  const err = body && body.error;
  expect(err, `${ctx}: error`).toBeDefined();
  expect(err.code, `${ctx}: error.code`).toBe(expectedCode);
  expect(err.message, `${ctx}: error.message`).toBe(expectedMessage);
}

export function parseAccessToken(res, ctx) {
  const body = parseJson(res, ctx, { redactAccessToken: true });
  expect(body, `${ctx}: token body`).toHaveProperty("access_token");
  expect(typeof body.access_token, `${ctx}: access_token type`).toBe("string");
  expect(
    body.access_token.indexOf("."),
    `${ctx}: access_token shape`,
  ).toBeGreaterThan(0);
  return body.access_token;
}
