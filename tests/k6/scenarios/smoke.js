import { group } from "k6";

import { bucketSetupRestrictedAndSigned, uniqueKey } from "../lib/data.js";
import { cleanupEnabled } from "../lib/env.js";
import {
  createBucket,
  deleteBucket,
  deleteKey,
  getKey,
  getReady,
  mintToken,
  putKey,
  runTransaction,
  scrapeMetrics,
} from "../lib/api.js";
import { parseAccessToken } from "../lib/assertions.js";
import { writeSummary } from "../lib/summary.js";
import { must, parseJson } from "../lib/client.js";

export const options = {
  vus: 1,
  iterations: 1,
  thresholds: {
    checks: ["rate==1.0"],
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
  let bid = "";

  try {
    group("ops: readiness", () => {
      const res = getReady({ scenario: "smoke", flow: "readiness" });
      must(
        res,
        {
          "ready: status=200": (r) => r.status === 200,
        },
        "GET /ready",
      );
      const body = parseJson(res, "GET /ready");
      if (
        body.status !== "ok" ||
        body.storage !== "ok" ||
        typeof body.version !== "string"
      ) {
        throw new Error(`unexpected /ready body: ${JSON.stringify(body)}`);
      }
    });

    bid = group("bucket: create", () => {
      const res = createBucket(bucket, {
        scenario: "smoke",
        flow: "bucket_create",
      });
      must(
        res,
        {
          "create bucket: status=200": (r) => r.status === 200,
          "create bucket: body is uuid": (r) =>
            /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(
              String(r.body || "").trim(),
            ),
        },
        "POST /api/v1 (create bucket)",
      );
      return String(res.body).trim();
    });

    const msgPath = `/api/v1/${bid}/msg`;

    group("keys: put/get roundtrip (static keys)", () => {
      const put = putKey(bid, "msg", "hello", bucket.write_key, {
        scenario: "smoke",
        flow: "keys_roundtrip",
      });
      must(
        put,
        {
          "put: status=200": (r) => r.status === 200,
          "put: content-type text/plain": (r) =>
            r.headers["Content-Type"] === "text/plain; charset=utf-8",
          "put: body echoes": (r) => r.body === "hello",
        },
        `PUT ${msgPath}`,
      );

      const getRes = getKey(bid, "msg", bucket.read_key, {
        scenario: "smoke",
        flow: "keys_roundtrip",
      });
      must(
        getRes,
        {
          "get: status=200": (r) => r.status === 200,
          "get: content-type text/plain": (r) =>
            r.headers["Content-Type"] === "text/plain; charset=utf-8",
          "get: body matches": (r) => r.body === "hello",
        },
        `GET ${msgPath}`,
      );
    });

    const token = group("tokens: mint + scoped read", () => {
      const res = mintToken(
        bid,
        bucket.secret_key,
        {
          prefix: "scope:",
          permissions: "read",
          ttl: "3600",
        },
        { scenario: "smoke", flow: "token_mint" },
      );
      must(
        res,
        {
          "mint token: status=200": (r) => r.status === 200,
          "mint token: body includes access_token field": (r) =>
            /"access_token"\s*:\s*"/.test(String(r.body)),
        },
        `POST /api/v1/${bid}/tokens/`,
        { redactAccessToken: true },
      );
      return parseAccessToken(res, "mint token");
    });

    group("keys: scoped token reads prefixed key", () => {
      const scopedKey = uniqueKey("scope:k1");
      const scopedPath = `/api/v1/${bid}/${scopedKey}`;

      const put = putKey(bid, scopedKey, "v1", bucket.secret_key, {
        scenario: "smoke",
        flow: "scoped_token_read",
      });
      must(
        put,
        { "put scoped: status=200": (r) => r.status === 200 },
        `PUT ${scopedPath}`,
      );

      const getRes = getKey(bid, scopedKey, token, {
        scenario: "smoke",
        flow: "scoped_token_read",
      });
      must(
        getRes,
        {
          "get scoped: status=200": (r) => r.status === 200,
          "get scoped: body=v1": (r) => r.body === "v1",
        },
        `GET ${scopedPath}`,
      );
    });

    group("txn: set then read", () => {
      const txn = runTransaction(
        bid,
        bucket.secret_key,
        { txn: [{ set: "txn:k", value: "tv" }] },
        { scenario: "smoke", flow: "transaction" },
      );
      must(
        txn,
        { "txn: status=204": (r) => r.status === 204 },
        `POST /api/v1/${bid} (txn)`,
      );

      const getRes = getKey(bid, "txn:k", bucket.secret_key, {
        scenario: "smoke",
        flow: "transaction",
      });
      must(
        getRes,
        {
          "txn read: status=200": (r) => r.status === 200,
          "txn read: body=tv": (r) => r.body === "tv",
        },
        `GET /api/v1/${bid}/txn:k`,
      );
    });

    group("keys: delete", () => {
      const res = deleteKey(bid, "msg", bucket.secret_key, {
        scenario: "smoke",
        flow: "key_delete",
      });
      must(
        res,
        { "delete: status=204": (r) => r.status === 204 },
        `DELETE ${msgPath}`,
      );
    });

    group("ops: metrics scrape", () => {
      const res = scrapeMetrics({ scenario: "smoke", flow: "metrics" });
      must(
        res,
        {
          "metrics: status=200": (r) => r.status === 200,
          "metrics: content-type prometheus": (r) =>
            r.headers["Content-Type"] ===
            "text/plain; version=0.0.4; charset=utf-8",
          "metrics: advertises http_requests_total": (r) =>
            String(r.body).indexOf("# HELP http_requests_total ") !== -1,
          "metrics: advertises storage_ops_total": (r) =>
            String(r.body).indexOf("# HELP storage_ops_total ") !== -1,
        },
        "GET /metrics",
      );
    });
  } finally {
    if (bid && cleanupEnabled()) {
      const res = deleteBucket(bid, bucket.secret_key, {
        scenario: "smoke",
        flow: "cleanup",
      });
      if (res.status !== 204 && res.status !== 404) {
        throw new Error(`bucket cleanup failed (status=${res.status})`);
      }
    }
  }
}
