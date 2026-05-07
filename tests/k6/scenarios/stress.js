import { fail } from "k6";
import { Rate, Trend } from "k6/metrics";

import { bucketSetupRestrictedAndSigned } from "../lib/data.js";
import { cleanupEnabled } from "../lib/env.js";
import { deleteBucket, deleteKey, getKey, putKey } from "../lib/api.js";
import { createRestrictedBucket } from "../lib/contract.js";
import { writeSummary } from "../lib/summary.js";

const SCENARIO = "stress";

export const keyRoundtripDuration = new Trend(
  "stress_key_roundtrip_duration",
  true,
);
export const putOk = new Rate("stress_put_ok");
export const getOk = new Rate("stress_get_ok");
export const deleteOk = new Rate("stress_delete_ok");

function numEnv(name, fallback) {
  const raw = __ENV[name];
  if (raw === undefined || raw === null || raw === "") return fallback;
  const n = Number(raw);
  if (!Number.isFinite(n) || n < 0) return fallback;
  return n;
}

export const options = {
  scenarios: {
    stress: {
      executor: "ramping-vus",
      stages: [
        {
          duration: __ENV.STRESS_RAMP_UP || "15s",
          target: numEnv("STRESS_MAX_VUS", 40),
        },
        {
          duration: __ENV.STRESS_HOLD || "15s",
          target: numEnv("STRESS_MAX_VUS", 40),
        },
        { duration: __ENV.STRESS_RAMP_DOWN || "10s", target: 0 },
      ],
      gracefulStop: "10s",
    },
  },
  thresholds: {
    // Stress is expected to push limits; keep defaults permissive and tighten via env/CI if desired.
    http_req_failed: ["rate<0.10"],
    http_req_duration: ["p(95)<2000"],
    stress_key_roundtrip_duration: ["p(95)<2500"],
  },
};

export function handleSummary(data) {
  return writeSummary("stress", data);
}

let transportErrorsInARow = 0;

export default function stress(data) {
  const bucket = data.bucket;
  const bid = data.bid;
  const startedAt = Date.now();
  const abortAfter = numEnv("STRESS_ABORT_TRANSPORT_ERRORS", 25);

  function observeTransport(status) {
    if (status === 0) {
      transportErrorsInARow += 1;
    } else {
      transportErrorsInARow = 0;
    }

    // When the target is down (connection refused / EOF), k6 will emit a warning per failed request.
    // Abort early so we don't flood stdout and we get a clean saturation signal.
    if (transportErrorsInARow >= abortAfter) {
      fail(
        `aborting stress: ${transportErrorsInARow} transport errors in a row`,
      );
    }
  }

  // Keep URLs low-cardinality so k6 doesn't blow up metrics with unique time series.
  // Use one stable key per VU to keep the dataset bounded.
  const key = `stress:k-${__ENV.RUN_ID || "run"}-vu${__VU}`;

  const put = putKey(bid, key, "x", bucket.write_key, {
    scenario: SCENARIO,
    flow: "key_roundtrip",
  });
  observeTransport(put.status);
  putOk.add(put.status === 200);

  if (put.status !== 200) return;

  const g = getKey(bid, key, bucket.read_key, {
    scenario: SCENARIO,
    flow: "key_roundtrip",
  });
  observeTransport(g.status);
  getOk.add(g.status === 200);

  const d = deleteKey(bid, key, bucket.secret_key, {
    scenario: SCENARIO,
    flow: "key_roundtrip",
  });
  observeTransport(d.status);
  deleteOk.add(d.status === 204);

  // Record duration even if GET/DELETE failed; it still reflects end-to-end pressure on the system.
  keyRoundtripDuration.add(Date.now() - startedAt);
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
