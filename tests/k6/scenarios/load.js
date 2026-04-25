import { Trend } from "k6/metrics";

import { bucketSetupRestrictedAndSigned } from "../lib/data.js";
import { cleanupEnabled } from "../lib/env.js";
import {
  createBucket,
  deleteBucket,
  deleteKey,
  getKey,
  putKey,
} from "../lib/api.js";
import { must } from "../lib/client.js";
import { writeSummary } from "../lib/summary.js";

export const keyRoundtripDuration = new Trend("key_roundtrip_duration", true);

export const options = {
  scenarios: {
    load: {
      executor: "constant-vus",
      vus: Number(__ENV.LOAD_VUS || 10),
      duration: __ENV.LOAD_DURATION || "30s",
      gracefulStop: "5s",
    },
  },
  thresholds: {
    checks: ["rate>0.999"],
    http_req_failed: ["rate<0.01"],
    http_req_duration: ["p(95)<500"],
    "http_req_duration{name:PUT /api/v1/:bucket/:key}": ["p(95)<500"],
    "http_req_duration{name:GET /api/v1/:bucket/:key}": ["p(95)<500"],
    key_roundtrip_duration: ["p(95)<750"],
  },
};

export function handleSummary(data) {
  return writeSummary("load", data);
}

export function setup() {
  const bucket = bucketSetupRestrictedAndSigned();
  const res = createBucket(bucket, { scenario: "load", flow: "setup" });
  must(
    res,
    { "create bucket: status=200": (r) => r.status === 200 },
    "POST /api/v1 (load setup)",
  );
  const bid = String(res.body).trim();
  return { bucket: bucket, bid: bid };
}

export default function load(data) {
  const bucket = data.bucket;
  const bid = data.bid;
  const startedAt = Date.now();

  // Keep URLs low-cardinality so k6 doesn't blow up metrics with unique time series.
  const key = `load:k-${__ENV.RUN_ID || "run"}-vu${__VU}`;

  const put = putKey(bid, key, "x", bucket.write_key, {
    scenario: "load",
    flow: "key_roundtrip",
  });
  must(
    put,
    { "put: 200": (r) => r.status === 200 },
    `PUT /api/v1/${bid}/${key}`,
  );

  const g = getKey(bid, key, bucket.read_key, {
    scenario: "load",
    flow: "key_roundtrip",
  });
  must(g, { "get: 200": (r) => r.status === 200 }, `GET /api/v1/${bid}/${key}`);

  // Keep the dataset bounded so long runs don't degrade.
  const d = deleteKey(bid, key, bucket.secret_key, {
    scenario: "load",
    flow: "key_roundtrip",
  });
  must(
    d,
    { "delete: 204": (r) => r.status === 204 },
    `DELETE /api/v1/${bid}/${key}`,
  );

  keyRoundtripDuration.add(Date.now() - startedAt);
}

export function teardown(data) {
  if (!cleanupEnabled()) return;
  const bucket = data.bucket;
  const bid = data.bid;
  const res = deleteBucket(bid, bucket.secret_key, {
    scenario: "load",
    flow: "cleanup",
  });
  if (res.status !== 204 && res.status !== 404) {
    throw new Error(`bucket cleanup failed (status=${res.status})`);
  }
}
