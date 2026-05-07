import { group } from "k6";

import { bucketSetupRestrictedAndSigned, uniqueKey } from "../lib/data.js";
import { cleanupEnabled } from "../lib/env.js";
import { patchBucket, putKey, putKeyWithoutAuth } from "../lib/api.js";
import { assertApiError } from "../lib/assertions.js";
import {
  cleanupBuckets,
  createRestrictedBucket,
  getKeyApiError,
  getTextKey,
  mintTokenChecked,
  putTextKey,
} from "../lib/contract.js";
import { checkRes, tags } from "../lib/scenario.js";
import { expect } from "../lib/testing.js";
import { writeSummary } from "../lib/summary.js";

const SCENARIO = "security";

function t(flow) {
  return tags(SCENARIO, flow);
}

export const options = {
  vus: 1,
  iterations: 1,
  thresholds: {
    http_req_failed: ["rate==0"],
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
  const createdBuckets = [];
  let bidA = "";
  let bidB = "";

  try {
    bidA = group("setup: create bucket A", () => {
      return createRestrictedBucket({
        scenario: SCENARIO,
        flow: "setup_A",
        form: a,
        createdBuckets,
      });
    });

    bidB = group("setup: create bucket B", () => {
      return createRestrictedBucket({
        scenario: SCENARIO,
        flow: "setup_B",
        form: b,
        createdBuckets,
      });
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
      return mintTokenChecked({
        scenario: SCENARIO,
        flow: "token_mint",
        bid: bidA,
        secretKey: a.secret_key,
        body: { prefix: "scope:", permissions: "read", ttl: "3600" },
      });
    });

    group("tokens: prefix is enforced (403 outside scope)", () => {
      getKeyApiError({
        scenario: SCENARIO,
        flow: "token_prefix",
        bid: bidA,
        key: "admin:config",
        token,
        expectedStatus: 403,
        expectedCode: 403,
        expectedMessage: "forbidden",
        ctx: `GET /api/v1/${bidA}/admin:config (token outside prefix)`,
      });
    });

    group("tokens: wrong bucket is 401", () => {
      getKeyApiError({
        scenario: SCENARIO,
        flow: "token_bucket_isolation",
        bid: bidB,
        key: "scope:k1",
        token,
        expectedStatus: 401,
        expectedCode: 401,
        expectedMessage: "unauthorized",
        ctx: `GET /api/v1/${bidB}/scope:k1 (token for other bucket)`,
      });
    });

    group("tokens: invalidated after signing_key rotation", () => {
      const insideKey = uniqueKey("scope:probe");
      const insidePath = `/api/v1/${bidA}/${insideKey}`;

      // Seed a key under the token scope, so rejection after rotation exercises signature verification.
      putTextKey({
        scenario: SCENARIO,
        flow: "token_rotation",
        bid: bidA,
        key: insideKey,
        value: "v",
        token: a.secret_key,
      });

      getTextKey({
        scenario: SCENARIO,
        flow: "token_rotation",
        bid: bidA,
        key: insideKey,
        expectedBody: "v",
        token,
      });

      const newSigning = `sign-rot-${insideKey}`;
      const patch = patchBucket(
        bidA,
        a.secret_key,
        { signing_key: newSigning },
        t("token_rotation"),
      );
      checkRes(patch, `PATCH /api/v1/${bidA} (rotate signing_key)`, () => {
        expect(patch.status).toBe(204);
      });

      getKeyApiError({
        scenario: SCENARIO,
        flow: "token_rotation",
        bid: bidA,
        key: insideKey,
        token,
        expectedStatus: 401,
        expectedCode: 401,
        expectedMessage: "unauthorized",
        ctx: `GET ${insidePath} (token after rotation)`,
      });

      const tokenAfterRotation = mintTokenChecked({
        scenario: SCENARIO,
        flow: "token_rotation",
        bid: bidA,
        secretKey: a.secret_key,
        body: { prefix: "scope:", permissions: "read", ttl: "3600" },
      });

      getTextKey({
        scenario: SCENARIO,
        flow: "token_rotation",
        bid: bidA,
        key: insideKey,
        expectedBody: "v",
        token: tokenAfterRotation,
      });
    });
  } finally {
    if (cleanupEnabled()) {
      cleanupBuckets({ scenario: SCENARIO, createdBuckets });
    }
  }
}
