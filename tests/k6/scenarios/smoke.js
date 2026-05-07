import { group } from "k6";

import { bucketSetupRestrictedAndSigned, uniqueKey } from "../lib/data.js";
import { cleanupEnabled } from "../lib/env.js";
import {
  deleteKey,
  getReady,
  runTransaction,
  scrapeMetrics,
} from "../lib/api.js";
import { assertContentType } from "../lib/assertions.js";
import {
  cleanupBuckets,
  createRestrictedBucket,
  getTextKey,
  mintTokenChecked,
  putTextKey,
} from "../lib/contract.js";
import { checkRes, tags } from "../lib/scenario.js";
import { expect } from "../lib/testing.js";
import { writeSummary } from "../lib/summary.js";
import { parseJson } from "../lib/client.js";

const SCENARIO = "smoke";

function t(flow) {
  return tags(SCENARIO, flow);
}

export const options = {
  vus: 1,
  iterations: 1,
  thresholds: {
    http_req_failed: ["rate==0"],
    "http_req_duration{name:GET /ready}": ["p(95)<200"],
    "http_req_duration{name:POST /api/v1/:bucket/tokens/}": ["p(95)<500"],
    "group_duration{group:::tokens: mint + scoped read}": ["p(95)<700"],
  },
};

export function handleSummary(data) {
  return writeSummary("smoke", data);
}

export default function smoke() {
  const bucket = bucketSetupRestrictedAndSigned();
  const createdBuckets = [];
  let bid = "";

  try {
    group("ops: readiness", () => {
      const res = getReady(t("readiness"));
      checkRes(res, "GET /ready", () => {
        expect(res.status, "ready status").toBe(200);
        assertContentType(res, "application/json", "GET /ready");
        const body = parseJson(res, "GET /ready");
        expect(body).toHaveProperty("status", "ok");
        expect(body).toHaveProperty("storage", "ok");
        expect(typeof body.version, "/ready.version type").toBe("string");
      });
    });

    bid = group("bucket: create", () => {
      return createRestrictedBucket({
        scenario: SCENARIO,
        flow: "bucket_create",
        form: bucket,
        createdBuckets,
      });
    });

    const msgPath = `/api/v1/${bid}/msg`;

    group("keys: put/get roundtrip (static keys)", () => {
      putTextKey({
        scenario: SCENARIO,
        flow: "keys_roundtrip",
        bid,
        key: "msg",
        value: "hello",
        token: bucket.write_key,
      });
      getTextKey({
        scenario: SCENARIO,
        flow: "keys_roundtrip",
        bid,
        key: "msg",
        expectedBody: "hello",
        token: bucket.read_key,
      });
    });

    const token = group("tokens: mint + scoped read", () => {
      return mintTokenChecked({
        scenario: SCENARIO,
        flow: "token_mint",
        bid,
        secretKey: bucket.secret_key,
        body: { prefix: "scope:", permissions: "read", ttl: "3600" },
      });
    });

    group("keys: scoped token reads prefixed key", () => {
      const scopedKey = uniqueKey("scope:k1");

      putTextKey({
        scenario: SCENARIO,
        flow: "scoped_token_read",
        bid,
        key: scopedKey,
        value: "v1",
        token: bucket.secret_key,
      });

      getTextKey({
        scenario: SCENARIO,
        flow: "scoped_token_read",
        bid,
        key: scopedKey,
        expectedBody: "v1",
        token,
      });
    });

    group("txn: set then read", () => {
      const txn = runTransaction(
        bid,
        bucket.secret_key,
        { txn: [{ set: "txn:k", value: "tv" }] },
        t("transaction"),
      );
      checkRes(txn, `POST /api/v1/${bid} (txn)`, () => {
        expect(txn.status).toBe(204);
      });

      getTextKey({
        scenario: SCENARIO,
        flow: "transaction",
        bid,
        key: "txn:k",
        expectedBody: "tv",
        token: bucket.secret_key,
      });
    });

    group("keys: delete", () => {
      const res = deleteKey(bid, "msg", bucket.secret_key, t("key_delete"));
      checkRes(res, `DELETE ${msgPath}`, () => {
        expect(res.status).toBe(204);
      });
    });

    group("ops: metrics scrape", () => {
      const res = scrapeMetrics(t("metrics"));
      checkRes(res, "GET /metrics", () => {
        expect(res.status).toBe(200);
        assertContentType(
          res,
          "text/plain; version=0.0.4; charset=utf-8",
          "GET /metrics",
        );
        expect(String(res.body)).toContain("# HELP http_requests_total ");
        expect(String(res.body)).toContain("# HELP storage_ops_total ");
      });
    });
  } finally {
    if (cleanupEnabled()) {
      cleanupBuckets({ scenario: SCENARIO, createdBuckets });
    }
  }
}
