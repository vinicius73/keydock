import {
  createBucket,
  deleteBucket,
  getKey,
  headKey,
  mintToken,
  postKey,
  putKey,
  putKeyBytes,
  putKeyJson,
} from "./api.js";
import { parseJson } from "./client.js";
import {
  assertApiError,
  assertBinaryEquals,
  assertContentType,
  assertEmptyBody,
  assertJsonEquals,
  parseAccessToken,
} from "./assertions.js";
import { expect } from "./testing.js";
import { checkRes, tags } from "./scenario.js";

function isUuidText(s) {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
    String(s || "").trim(),
  );
}

export function createRestrictedBucket({
  scenario,
  flow,
  form,
  createdBuckets,
}) {
  // We track created buckets so scenarios can clean up deterministically when K6_CLEANUP is enabled.
  const res = createBucket(form, tags(scenario, flow));
  checkRes(res, "POST /api/v1 (create bucket)", () => {
    expect(res.status, "status").toBe(200);
    assertContentType(res, "text/plain; charset=utf-8", "create bucket");
    expect(isUuidText(res.body), "body is uuid").toBeTruthy();
  });
  const bid = String(res.body).trim();
  if (createdBuckets)
    createdBuckets.push({ bid: bid, secret: form.secret_key });
  return bid;
}

export function cleanupBuckets({ scenario, createdBuckets }) {
  for (const b of createdBuckets) {
    // Cleanup is best-effort and must never hide contract failures earlier in the run.
    const res = deleteBucket(b.bid, b.secret, tags(scenario, "cleanup"));
    if (res.status !== 204 && res.status !== 404) {
      throw new Error(
        `bucket cleanup failed (bid=${b.bid} status=${res.status})`,
      );
    }
  }
}

export function putTextKey({
  scenario,
  flow,
  bid,
  key,
  value,
  token,
  expectedContentType = "text/plain; charset=utf-8",
}) {
  const path = `/api/v1/${bid}/${key}`;
  const res = putKey(bid, key, value, token, tags(scenario, flow));
  return checkRes(res, `PUT ${path}`, () => {
    expect(res.status, "status").toBe(200);
    assertContentType(res, expectedContentType, `PUT ${path}`);
    expect(res.body, "body").toBe(value);
  });
}

export function postTextKey({ scenario, flow, bid, key, value, token }) {
  const path = `/api/v1/${bid}/${key}`;
  const res = postKey(bid, key, value, token, tags(scenario, flow));
  return checkRes(res, `POST ${path}`, () => {
    expect(res.status, "status").toBe(200);
    expect(res.body, "body").toBe(value);
  });
}

export function getTextKey({
  scenario,
  flow,
  bid,
  key,
  token,
  expectedBody,
  expectedContentType = "text/plain; charset=utf-8",
}) {
  const path = `/api/v1/${bid}/${key}`;
  const res = getKey(bid, key, token, tags(scenario, flow));
  return checkRes(res, `GET ${path}`, () => {
    expect(res.status, "status").toBe(200);
    assertContentType(res, expectedContentType, `GET ${path}`);
    expect(res.body, "body").toBe(expectedBody);
  });
}

export function putJsonKey({ scenario, flow, bid, key, value, token }) {
  const path = `/api/v1/${bid}/${key}`;
  const res = putKeyJson(
    bid,
    key,
    JSON.stringify(value),
    token,
    tags(scenario, flow),
  );
  return checkRes(res, `PUT ${path}`, () => {
    expect(res.status, "status").toBe(200);
    assertContentType(res, "application/json", `PUT ${path}`);
    assertJsonEquals(parseJson(res, `PUT ${path}`), value, `PUT ${path} body`);
  });
}

export function getJsonKey({ scenario, flow, bid, key, token, expected }) {
  const path = `/api/v1/${bid}/${key}`;
  const res = getKey(bid, key, token, tags(scenario, flow));
  return checkRes(res, `GET ${path}`, () => {
    expect(res.status, "status").toBe(200);
    assertContentType(res, "application/json", `GET ${path}`);
    assertJsonEquals(
      parseJson(res, `GET ${path}`),
      expected,
      `GET ${path} body`,
    );
  });
}

export function headKeyContract({
  scenario,
  flow,
  bid,
  key,
  token,
  expectedContentType,
}) {
  const path = `/api/v1/${bid}/${key}`;
  const res = headKey(bid, key, token, tags(scenario, flow), {
    expectedStatus: 200,
  });
  return checkRes(res, `HEAD ${path}`, () => {
    expect(res.status, "status").toBe(200);
    assertContentType(res, expectedContentType, `HEAD ${path}`);
    // HEAD responses must never leak content; only headers are meaningful here.
    assertEmptyBody(res, `HEAD ${path}`);
  });
}

export function putBinaryKey({ scenario, flow, bid, key, bytes, token }) {
  const path = `/api/v1/${bid}/${key}`;
  const res = putKeyBytes(
    bid,
    key,
    bytes,
    token,
    tags(scenario, flow),
    undefined,
    {
      responseType: "binary",
      headers: { "Content-Type": "application/octet-stream" },
    },
  );
  return checkRes(res, `PUT ${path}`, () => {
    expect(res.status, "status").toBe(200);
    assertContentType(res, "application/octet-stream", `PUT ${path}`);
    assertBinaryEquals(res, bytes, `PUT ${path} body`);
  });
}

export function getBinaryKey({
  scenario,
  flow,
  bid,
  key,
  token,
  expectedBytes,
}) {
  const path = `/api/v1/${bid}/${key}`;
  const res = getKey(bid, key, token, tags(scenario, flow), undefined, {
    responseType: "binary",
  });
  return checkRes(res, `GET ${path}`, () => {
    expect(res.status, "status").toBe(200);
    assertContentType(res, "application/octet-stream", `GET ${path}`);
    assertBinaryEquals(res, expectedBytes, `GET ${path} body`);
  });
}

export function mintTokenChecked({ scenario, flow, bid, secretKey, body }) {
  const res = mintToken(bid, secretKey, body, tags(scenario, flow));
  checkRes(
    res,
    `POST /api/v1/${bid}/tokens/`,
    () => {
      expect(res.status, "status").toBe(200);
      assertContentType(res, "application/json", "mint token");
      parseAccessToken(res, "mint token");
    },
    { redactAccessToken: true },
  );
  return parseAccessToken(res, "mint token");
}

export function getKeyApiError({
  scenario,
  flow,
  bid,
  key,
  token,
  expectedStatus,
  expectedCode,
  expectedMessage,
  ctx,
}) {
  const path = `/api/v1/${bid}/${key}`;
  const res = getKey(bid, key, token, tags(scenario, flow), { expectedStatus });
  assertApiError(res, expectedCode, expectedMessage, ctx || `GET ${path}`);
  return res;
}
