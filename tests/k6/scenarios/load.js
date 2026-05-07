import { Trend } from "k6/metrics";

import { bucketSetupRestrictedAndSigned } from "../lib/data.js";
import { cleanupEnabled } from "../lib/env.js";
import { deleteBucket, deleteKey, getKey, putKey } from "../lib/api.js";
import { createRestrictedBucket } from "../lib/contract.js";
import { expect } from "../lib/testing.js";
import { writeSummary } from "../lib/summary.js";

const SCENARIO = "load";

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
  const bid = createRestrictedBucket({
    scenario: SCENARIO,
    flow: "setup",
    form: bucket,
  });
  return { bucket: bucket, bid: bid };
}

export default function load(data) {
  const bucket = data.bucket;
  const bid = data.bid;
  const startedAt = Date.now();

  // Keep URLs low-cardinality so k6 doesn't blow up metrics with unique time series.
  const key = `load:k-${__ENV.RUN_ID || "run"}-vu${__VU}`;

  // Load is intentionally tolerant: use soft assertions so transient failures show up in metrics
  // without aborting the whole run early (which would skew results).
  const put = putKey(bid, key, "x", bucket.write_key, {
    scenario: SCENARIO,
    flow: "key_roundtrip",
  });
  if (put.status !== 200) {
    expect.soft(put.status, `PUT /api/v1/${bid}/${key} status`).toBe(200);
    return;
  }

  const g = getKey(bid, key, bucket.read_key, {
    scenario: SCENARIO,
    flow: "key_roundtrip",
  });
  if (g.status !== 200) {
    expect.soft(g.status, `GET /api/v1/${bid}/${key} status`).toBe(200);
  }

  // Keep the dataset bounded so long runs don't degrade.
  const d = deleteKey(bid, key, bucket.secret_key, {
    scenario: SCENARIO,
    flow: "key_roundtrip",
  });
  expect.soft(d.status, `DELETE /api/v1/${bid}/${key} status`).toBe(204);

  keyRoundtripDuration.add(Date.now() - startedAt);
}

export function teardown(data) {
  if (!cleanupEnabled()) return;
  const bucket = data.bucket;
  const bid = data.bid;
  const res = deleteBucket(bid, bucket.secret_key, {
    scenario: SCENARIO,
    flow: "cleanup",
  });
  if (res.status !== 204 && res.status !== 404) {
    throw new Error(`bucket cleanup failed (status=${res.status})`);
  }
}
