import { group } from "k6";

import { bucketSetupRestrictedAndSigned, uniqueKey } from "../lib/data.js";
import { cleanupEnabled } from "../lib/env.js";
import {
  createBucket,
  deleteBucket,
  deleteKey,
  getKey,
  listKeysJson,
  putKey,
  runTransaction,
} from "../lib/api.js";
import { assertApiError } from "../lib/assertions.js";
import { must, parseJson } from "../lib/client.js";
import { writeSummary } from "../lib/summary.js";

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
    checks: ["rate==1.0"],
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
  let bid = "";

  try {
    bid = group("setup: create bucket", () => {
      const res = createBucket(bucket, {
        scenario: "regression",
        flow: "setup",
      });
      must(
        res,
        { "create bucket: status=200": (r) => r.status === 200 },
        "POST /api/v1",
      );
      return String(res.body).trim();
    });

    const k1 = uniqueKey("k1");
    const k2 = uniqueKey("k2");
    const p1 = `/api/v1/${bid}/${k1}`;
    const p2 = `/api/v1/${bid}/${k2}`;

    group("keys: put/get", () => {
      must(
        putKey(bid, k1, "v1", bucket.write_key, {
          scenario: "regression",
          flow: "keys_roundtrip",
        }),
        { "put: status=200": (r) => r.status === 200 },
        `PUT ${p1}`,
      );

      const g = getKey(bid, k1, bucket.read_key, {
        scenario: "regression",
        flow: "keys_roundtrip",
      });
      must(
        g,
        {
          "get: status=200": (r) => r.status === 200,
          "get: body=v1": (r) => r.body === "v1",
        },
        `GET ${p1}`,
      );
    });

    group("listing: includes written key (json)", () => {
      const list = listKeysJson(bid, bucket.read_key, {
        scenario: "regression",
        flow: "list_keys",
      });
      must(
        list,
        {
          "list: status=200": (r) => r.status === 200,
          "list: content-type json": (r) =>
            r.headers["Content-Type"] === "application/json",
        },
        `GET /api/v1/${bid}/`,
      );
      const body = parseJson(list, `GET /api/v1/${bid}/`);
      if (!Array.isArray(body) || body.indexOf(k1) === -1) {
        throw new Error(
          `list: expected array containing ${k1}, got ${JSON.stringify(body)}`,
        );
      }
    });

    group("txn: invalid shape is rejected and does not mutate", () => {
      const bad = runTransaction(
        bid,
        bucket.secret_key,
        { txn: [{ set: k2 }] }, // missing value
        { scenario: "regression", flow: "invalid_txn" },
        { expectedStatus: 400 },
      );
      assertApiError(bad, 400, "bad_request", `POST /api/v1/${bid} (bad txn)`);

      const g = getKey(
        bid,
        k2,
        bucket.secret_key,
        {
          scenario: "regression",
          flow: "invalid_txn",
        },
        {
          expectedStatus: 404,
        },
      );
      assertApiError(g, 404, "not_found", `GET ${p2} (after bad txn)`);
    });

    group("cleanup: delete existing key", () => {
      const d = deleteKey(bid, k1, bucket.secret_key, {
        scenario: "regression",
        flow: "delete_key",
      });
      must(
        d,
        { "delete: status=204": (r) => r.status === 204 },
        `DELETE ${p1}`,
      );

      const g = getKey(
        bid,
        k1,
        bucket.secret_key,
        {
          scenario: "regression",
          flow: "delete_key",
        },
        {
          expectedStatus: 404,
        },
      );
      assertApiError(g, 404, "not_found", `GET ${p1} (after delete)`);
    });
  } finally {
    if (bid && cleanupEnabled()) {
      const res = deleteBucket(bid, bucket.secret_key, {
        scenario: "regression",
        flow: "cleanup",
      });
      if (res.status !== 204 && res.status !== 404) {
        throw new Error(`bucket cleanup failed (status=${res.status})`);
      }
    }
  }
}
