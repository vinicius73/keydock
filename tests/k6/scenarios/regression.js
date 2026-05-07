import { group } from "k6";

import { bucketSetupRestrictedAndSigned, uniqueKey } from "../lib/data.js";
import { cleanupEnabled } from "../lib/env.js";
import { deleteKey, listKeysJson, runTransaction } from "../lib/api.js";
import { assertApiError } from "../lib/assertions.js";
import { assertContentType } from "../lib/assertions.js";
import {
  cleanupBuckets,
  createRestrictedBucket,
  getKeyApiError,
  getTextKey,
  putTextKey,
} from "../lib/contract.js";
import { checkRes, tags } from "../lib/scenario.js";
import { parseJson } from "../lib/client.js";
import { expect } from "../lib/testing.js";
import { writeSummary } from "../lib/summary.js";

const SCENARIO = "regression";

function t(flow) {
  return tags(SCENARIO, flow);
}

export const options = {
  scenarios: {
    regression: {
      executor: "shared-iterations",
      vus: 5,
      iterations: 20,
      maxDuration: "2m",
    },
  },
  thresholds: {
    http_req_failed: ["rate==0"],
    "http_req_duration{name:GET /api/v1/:bucket/:key}": ["p(95)<500"],
    "http_req_duration{name:PUT /api/v1/:bucket/:key}": ["p(95)<500"],
    "http_req_duration{name:POST /api/v1/:bucket}": ["p(95)<500"],
  },
};

export function handleSummary(data) {
  return writeSummary("regression", data);
}

export default function regression() {
  const bucket = bucketSetupRestrictedAndSigned();
  const createdBuckets = [];
  let bid = "";

  try {
    bid = group("setup: create bucket", () => {
      return createRestrictedBucket({
        scenario: SCENARIO,
        flow: "setup",
        form: bucket,
        createdBuckets,
      });
    });

    const k1 = uniqueKey("k1");
    const k2 = uniqueKey("k2");
    const p1 = `/api/v1/${bid}/${k1}`;
    const p2 = `/api/v1/${bid}/${k2}`;

    group("keys: put/get", () => {
      putTextKey({
        scenario: SCENARIO,
        flow: "keys_roundtrip",
        bid,
        key: k1,
        value: "v1",
        token: bucket.write_key,
      });
      getTextKey({
        scenario: SCENARIO,
        flow: "keys_roundtrip",
        bid,
        key: k1,
        expectedBody: "v1",
        token: bucket.read_key,
      });
    });

    group("listing: includes written key (json)", () => {
      const list = listKeysJson(bid, bucket.read_key, t("list_keys"));
      checkRes(list, `GET /api/v1/${bid}/`, () => {
        expect(list.status).toBe(200);
        assertContentType(list, "application/json", "list keys json");
        const body = parseJson(list, `GET /api/v1/${bid}/`);
        expect(Array.isArray(body), "list keys: body is array").toBeTruthy();
        expect(body).toContain(k1);
      });
    });

    group("txn: invalid shape is rejected and does not mutate", () => {
      const bad = runTransaction(
        bid,
        bucket.secret_key,
        { txn: [{ set: k2 }] }, // missing value
        t("invalid_txn"),
        { expectedStatus: 400 },
      );
      assertApiError(bad, 400, "bad_request", `POST /api/v1/${bid} (bad txn)`);

      getKeyApiError({
        scenario: SCENARIO,
        flow: "invalid_txn",
        bid,
        key: k2,
        token: bucket.secret_key,
        expectedStatus: 404,
        expectedCode: 404,
        expectedMessage: "not_found",
        ctx: `GET ${p2} (after bad txn)`,
      });
    });

    group("cleanup: delete existing key", () => {
      const d = deleteKey(bid, k1, bucket.secret_key, t("delete_key"));
      checkRes(d, `DELETE ${p1}`, () => {
        expect(d.status).toBe(204);
      });

      getKeyApiError({
        scenario: SCENARIO,
        flow: "delete_key",
        bid,
        key: k1,
        token: bucket.secret_key,
        expectedStatus: 404,
        expectedCode: 404,
        expectedMessage: "not_found",
        ctx: `GET ${p1} (after delete)`,
      });
    });
  } finally {
    if (cleanupEnabled()) {
      cleanupBuckets({ scenario: SCENARIO, createdBuckets });
    }
  }
}
