import { group } from "k6";

import { bucketSetupRestrictedAndSigned, uniqueKey } from "../lib/data.js";
import { cleanupEnabled } from "../lib/env.js";
import {
  createBucket,
  deleteBucket,
  getKey,
  mintToken,
  patchBucket,
  putKey,
  putKeyWithoutAuth,
} from "../lib/api.js";
import { assertApiError, parseAccessToken } from "../lib/assertions.js";
import { must } from "../lib/client.js";
import { writeSummary } from "../lib/summary.js";

export const options = {
  vus: 1,
  iterations: 1,
  thresholds: {
    checks: ["rate==1.0"],
    "http_req_duration{name:POST /api/v1/:bucket/tokens/}": ["p(95)<500"],
    "http_req_duration{name:PATCH /api/v1/:bucket}": ["p(95)<500"],
    "group_duration{group:::tokens: invalidated after signing_key rotation}": [
      "p(95)<1500",
    ],
  },
};

export function handleSummary(data) {
  return writeSummary("security", data);
}

export default function security() {
  const a = bucketSetupRestrictedAndSigned();
  const b = bucketSetupRestrictedAndSigned();
  let bidA = "";
  let bidB = "";

  try {
    bidA = group("setup: create bucket A", () => {
      const res = createBucket(a, { scenario: "security", flow: "setup" });
      must(
        res,
        { "bucket A: status=200": (r) => r.status === 200 },
        "POST /api/v1 (A)",
      );
      return String(res.body).trim();
    });

    bidB = group("setup: create bucket B", () => {
      const res = createBucket(b, { scenario: "security", flow: "setup" });
      must(
        res,
        { "bucket B: status=200": (r) => r.status === 200 },
        "POST /api/v1 (B)",
      );
      return String(res.body).trim();
    });

    group("auth: missing bearer is 401", () => {
      const key = uniqueKey("k");
      const path = `/api/v1/${bidA}/${key}`;
      const res = putKeyWithoutAuth(
        bidA,
        key,
        "x",
        {
          scenario: "security",
          flow: "missing_auth",
        },
        {
          expectedStatus: 401,
        },
      );
      assertApiError(res, 401, "unauthorized", `PUT ${path} (no auth)`);
    });

    group("auth: wrong credential is 401", () => {
      const key = uniqueKey("k");
      const path = `/api/v1/${bidA}/${key}`;
      const res = putKey(
        bidA,
        key,
        "x",
        "wrong",
        {
          scenario: "security",
          flow: "wrong_auth",
        },
        {
          expectedStatus: 401,
        },
      );
      assertApiError(res, 401, "unauthorized", `PUT ${path} (wrong bearer)`);
    });

    group("authz: read credential cannot write (403)", () => {
      const key = uniqueKey("k");
      const path = `/api/v1/${bidA}/${key}`;
      const res = putKey(
        bidA,
        key,
        "x",
        a.read_key,
        {
          scenario: "security",
          flow: "read_cannot_write",
        },
        {
          expectedStatus: 403,
        },
      );
      assertApiError(res, 403, "forbidden", `PUT ${path} (read bearer)`);
    });

    const token = group("tokens: mint scoped read token", () => {
      const res = mintToken(
        bidA,
        a.secret_key,
        { prefix: "scope:", permissions: "read", ttl: "3600" },
        { scenario: "security", flow: "token_mint" },
      );
      must(
        res,
        {
          "mint token: status=200": (r) => r.status === 200,
          "mint token: has access_token field": (r) =>
            /"access_token"\s*:\s*"/.test(String(r.body)),
        },
        `POST /api/v1/${bidA}/tokens/`,
        { redactAccessToken: true },
      );
      return parseAccessToken(res, "mint token");
    });

    group("tokens: prefix is enforced (403 outside scope)", () => {
      const outside = `/api/v1/${bidA}/admin:config`;
      const res = getKey(
        bidA,
        "admin:config",
        token,
        {
          scenario: "security",
          flow: "token_prefix",
        },
        {
          expectedStatus: 403,
        },
      );
      assertApiError(
        res,
        403,
        "forbidden",
        `GET ${outside} (token outside prefix)`,
      );
    });

    group("tokens: wrong bucket is 401", () => {
      const path = `/api/v1/${bidB}/scope:k1`;
      const res = getKey(
        bidB,
        "scope:k1",
        token,
        {
          scenario: "security",
          flow: "token_bucket_isolation",
        },
        {
          expectedStatus: 401,
        },
      );
      assertApiError(
        res,
        401,
        "unauthorized",
        `GET ${path} (token for other bucket)`,
      );
    });

    group("tokens: invalidated after signing_key rotation", () => {
      const insideKey = uniqueKey("scope:probe");
      const insidePath = `/api/v1/${bidA}/${insideKey}`;

      // Seed a key under the token scope, so rejection after rotation exercises signature verification.
      must(
        putKey(bidA, insideKey, "v", a.secret_key, {
          scenario: "security",
          flow: "token_rotation",
        }),
        { "seed scoped key: status=200": (r) => r.status === 200 },
        `PUT ${insidePath} (seed)`,
      );

      const okBefore = getKey(bidA, insideKey, token, {
        scenario: "security",
        flow: "token_rotation",
      });
      must(
        okBefore,
        {
          "token works before rotation: status=200": (r) => r.status === 200,
          "token works before rotation: body=v": (r) => r.body === "v",
        },
        `GET ${insidePath}`,
      );

      const newSigning = `sign-rot-${insideKey}`;
      const patch = patchBucket(
        bidA,
        a.secret_key,
        { signing_key: newSigning },
        { scenario: "security", flow: "token_rotation" },
      );
      must(
        patch,
        {
          "rotate signing_key: status=204": (r) => r.status === 204,
        },
        `PATCH /api/v1/${bidA} (rotate signing_key)`,
      );

      const unauthorized = getKey(
        bidA,
        insideKey,
        token,
        {
          scenario: "security",
          flow: "token_rotation",
        },
        {
          expectedStatus: 401,
        },
      );
      assertApiError(
        unauthorized,
        401,
        "unauthorized",
        `GET ${insidePath} (token after rotation)`,
      );

      const tok2 = mintToken(
        bidA,
        a.secret_key,
        { prefix: "scope:", permissions: "read", ttl: "3600" },
        { scenario: "security", flow: "token_rotation" },
      );
      must(
        tok2,
        { "mint token after rotation: status=200": (r) => r.status === 200 },
        "mint token 2",
        {
          redactAccessToken: true,
        },
      );
      const tokenAfterRotation = parseAccessToken(tok2, "mint token 2");

      const okAfter = getKey(bidA, insideKey, tokenAfterRotation, {
        scenario: "security",
        flow: "token_rotation",
      });
      must(
        okAfter,
        {
          "token works after rotation: status=200": (r) => r.status === 200,
          "token works after rotation: body=v": (r) => r.body === "v",
        },
        `GET ${insidePath} (token after rotation, new token)`,
      );
    });
  } finally {
    if (bidA && cleanupEnabled()) {
      const d1 = deleteBucket(bidA, a.secret_key, {
        scenario: "security",
        flow: "cleanup",
      });
      if (d1.status !== 204 && d1.status !== 404) {
        throw new Error(`bucket A cleanup failed (status=${d1.status})`);
      }
    }
    if (bidB && cleanupEnabled()) {
      const d2 = deleteBucket(bidB, b.secret_key, {
        scenario: "security",
        flow: "cleanup",
      });
      if (d2.status !== 204 && d2.status !== 404) {
        throw new Error(`bucket B cleanup failed (status=${d2.status})`);
      }
    }
  }
}
