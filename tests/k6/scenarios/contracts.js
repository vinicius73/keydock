import { group } from "k6";

import { bucketSetupRestrictedAndSigned } from "../lib/data.js";
import { cleanupEnabled, RUN_ID } from "../lib/env.js";
import {
  getBucketPolicy,
  headBucket,
  listBucket,
  mintToken,
  patchKey,
  postReady,
  runTransaction,
} from "../lib/api.js";
import { parseJson } from "../lib/client.js";
import {
  assertApiError,
  assertContentType,
  assertEmptyBody,
  assertJsonEquals,
  assertNoSensitiveFields,
} from "../lib/assertions.js";
import {
  cleanupBuckets as cleanupBucketsContract,
  createRestrictedBucket as createRestrictedBucketContract,
  getBinaryKey as getBinaryKeyContract,
  getKeyApiError as getKeyApiErrorContract,
  getJsonKey as getJsonKeyContract,
  getTextKey as getTextKeyContract,
  headKeyContract as headKeyContractContract,
  mintTokenChecked as mintTokenCheckedContract,
  postTextKey as postTextKeyContract,
  putBinaryKey as putBinaryKeyContract,
  putJsonKey as putJsonKeyContract,
  putTextKey as putTextKeyContract,
} from "../lib/contract.js";
import { checkRes, tags as makeTags } from "../lib/scenario.js";
import { expect } from "../lib/testing.js";
import { writeSummary } from "../lib/summary.js";

const SCENARIO = "contracts";

// This scenario is intentionally "boring": one iteration, deterministic data, and explicit assertions.
// The goal is to lock down HTTP response contracts (status, content-type, body shape) as a regression net.
export const options = {
  vus: 1,
  iterations: 1,
  thresholds: {
    http_req_failed: ["rate==0"],
    "http_req_duration{name:GET /ready}": ["p(95)<200"],
    "http_req_duration{name:POST /api/v1}": ["p(95)<500"],
    "http_req_duration{name:GET /api/v1/:bucket/:key}": ["p(95)<500"],
    "http_req_duration{name:PUT /api/v1/:bucket/:key}": ["p(95)<500"],
    "http_req_duration{name:POST /api/v1/:bucket}": ["p(95)<750"],
  },
};

export function handleSummary(data) {
  return writeSummary("contracts", data);
}

function tags(flow) {
  return makeTags(SCENARIO, flow);
}

function createRestrictedBucket(args) {
  return createRestrictedBucketContract({ scenario: SCENARIO, ...args });
}

function cleanupBuckets(createdBuckets) {
  return cleanupBucketsContract({ scenario: SCENARIO, createdBuckets });
}

function putTextKey(args) {
  return putTextKeyContract({ scenario: SCENARIO, ...args });
}

function postTextKey(args) {
  return postTextKeyContract({ scenario: SCENARIO, ...args });
}

function getTextKey(args) {
  return getTextKeyContract({ scenario: SCENARIO, ...args });
}

function putJsonKey(args) {
  return putJsonKeyContract({ scenario: SCENARIO, ...args });
}

function getJsonKey(args) {
  return getJsonKeyContract({ scenario: SCENARIO, ...args });
}

function headKeyContract(args) {
  return headKeyContractContract({ scenario: SCENARIO, ...args });
}

function putBinaryKey(args) {
  return putBinaryKeyContract({ scenario: SCENARIO, ...args });
}

function getBinaryKey(args) {
  return getBinaryKeyContract({ scenario: SCENARIO, ...args });
}

function mintTokenChecked(args) {
  return mintTokenCheckedContract({ scenario: SCENARIO, ...args });
}

function getKeyApiError(args) {
  return getKeyApiErrorContract({ scenario: SCENARIO, ...args });
}

export default function contracts() {
  const base = `contracts-${RUN_ID}`;
  const bucket = bucketSetupRestrictedAndSigned();
  const createdBuckets = [];

  try {
    group("ops: method not allowed uses JSON envelope", () => {
      const res = postReady(tags("method_not_allowed"), {
        expectedStatus: 405,
      });
      assertApiError(res, 405, "method_not_allowed", "POST /ready");
    });

    const bid = createRestrictedBucket({
      createdBuckets,
      flow: "main",
      form: bucket,
    });

    group("bucket: policy public projection contract", () => {
      const path = `/api/v1/${bid}`;
      const res = getBucketPolicy(
        bid,
        bucket.secret_key,
        tags("bucket_policy"),
      );
      checkRes(res, `GET ${path}`, () => {
        expect(res.status, "status").toBe(200);
        assertContentType(res, "application/json", `GET ${path}`);

        const body = parseJson(res, `GET ${path}`);
        assertJsonEquals(
          body,
          {
            default_ttl: 604800,
            has_secret_key: true,
            has_read_key: true,
            has_write_key: true,
            has_signing_key: true,
            signing_key_generation: 0,
            anonymous_access: {
              read: false,
              write: false,
              enumerate: false,
              delete: false,
            },
          },
          "bucket policy body",
        );
        assertNoSensitiveFields(
          body,
          [
            "secret_key",
            "secret_key_hash",
            "read_key_hash",
            "write_key_hash",
            "signing_key",
          ],
          "bucket policy body",
        );
      });
    });

    group("bucket: head is admin-only and bodyless", () => {
      const path = `/api/v1/${bid}`;
      const res = headBucket(bid, bucket.secret_key, tags("bucket_head"), {
        expectedStatus: 200,
      });
      checkRes(res, `HEAD ${path}`, () => {
        expect(res.status, "status").toBe(200);
        assertEmptyBody(res, `HEAD ${path}`);
      });
    });

    group("keys: value kind matrix (PUT/GET + selected HEAD)", () => {
      const keyText = `a-text-${base}`;
      const keyInt = `b-int-${base}`;
      const keyFloat = `c-float-${base}`;
      const keyJson = `d-json-${base}`;
      const keyBool = `e-bool-${base}`;
      const keyRaw = `f-raw-${base}`;

      group("keys: utf8 text (text/plain)", () => {
        putTextKey({
          bid,
          key: keyText,
          value: "hello",
          token: bucket.write_key,
          flow: "keys_matrix",
        });
        getTextKey({
          bid,
          key: keyText,
          expectedBody: "hello",
          token: bucket.read_key,
          flow: "keys_matrix",
        });
      });

      group("keys: numeric text inference (int64/float64 → text/plain)", () => {
        // These values are stored as typed numbers internally, but the wire contract is still text/plain.
        putTextKey({
          bid,
          key: keyInt,
          value: "42",
          token: bucket.write_key,
          flow: "keys_matrix",
        });
        getTextKey({
          bid,
          key: keyInt,
          expectedBody: "42",
          token: bucket.read_key,
          flow: "keys_matrix",
        });

        putTextKey({
          bid,
          key: keyFloat,
          value: "3.14",
          token: bucket.write_key,
          flow: "keys_matrix",
        });
        getTextKey({
          bid,
          key: keyFloat,
          expectedBody: "3.14",
          token: bucket.read_key,
          flow: "keys_matrix",
        });
      });

      group("keys: JSON content (application/json)", () => {
        putJsonKey({
          bid,
          key: keyJson,
          value: { a: 1 },
          token: bucket.write_key,
          flow: "keys_matrix",
        });
        getJsonKey({
          bid,
          key: keyJson,
          expected: { a: 1 },
          token: bucket.read_key,
          flow: "keys_matrix",
        });
        headKeyContract({
          bid,
          key: keyJson,
          token: bucket.read_key,
          flow: "keys_matrix",
          expectedContentType: "application/json",
        });
      });

      group("keys: JSON boolean inference (application/json)", () => {
        // The backend infers JSON for boolean literals even if the request is text/plain.
        putTextKey({
          bid,
          key: keyBool,
          value: "true",
          token: bucket.write_key,
          flow: "keys_matrix",
          expectedContentType: "application/json",
        });
        getJsonKey({
          bid,
          key: keyBool,
          expected: true,
          token: bucket.read_key,
          flow: "keys_matrix",
        });
      });

      group("keys: raw binary (application/octet-stream)", () => {
        // Binary requires explicit responseType="binary" in k6, otherwise body is decoded as text.
        const rawBytes = new Uint8Array([0xff, 0xfe, 0x00, 0x01]);
        putBinaryKey({
          bid,
          key: keyRaw,
          bytes: rawBytes,
          token: bucket.write_key,
          flow: "keys_matrix",
        });
        getBinaryKey({
          bid,
          key: keyRaw,
          expectedBytes: rawBytes,
          token: bucket.read_key,
          flow: "keys_matrix",
        });
        headKeyContract({
          bid,
          key: keyRaw,
          token: bucket.read_key,
          flow: "keys_matrix",
          expectedContentType: "application/octet-stream",
        });
      });

      group("keys: POST alias behaves like PUT", () => {
        // Compatibility: POST is accepted as an alias for PUT on key writes.
        const keyPost = `g-post-${base}`;
        postTextKey({
          bid,
          key: keyPost,
          value: "data",
          token: bucket.write_key,
          flow: "keys_post_alias",
        });
        getTextKey({
          bid,
          key: keyPost,
          expectedBody: "data",
          token: bucket.read_key,
          flow: "keys_post_alias",
        });
      });
    });

    group("counter: PATCH contract", () => {
      const keyCounter = `ctr-${base}`;
      const keyFloatCounter = `ctrf-${base}`;
      const keyBad = `ctrbad-${base}`;

      const p1 = patchKey(
        bid,
        keyCounter,
        "+1",
        bucket.write_key,
        tags("counter"),
      );
      checkRes(p1, "PATCH counter +1", () => {
        expect(p1.status).toBe(200);
        assertContentType(p1, "text/plain; charset=utf-8", "PATCH counter +1");
        expect(p1.body).toBe("1");
      });

      putTextKey({
        bid,
        key: keyFloatCounter,
        value: "10",
        token: bucket.write_key,
        flow: "counter",
      });
      const p2 = patchKey(
        bid,
        keyFloatCounter,
        "+1.5",
        bucket.write_key,
        tags("counter"),
      );
      checkRes(p2, "PATCH counter +1.5", () => {
        expect(p2.status).toBe(200);
        assertContentType(
          p2,
          "text/plain; charset=utf-8",
          "PATCH counter +1.5",
        );
        expect(p2.body).toBe("11.5");
      });

      const bad = patchKey(
        bid,
        keyBad,
        "5",
        bucket.write_key,
        tags("counter"),
        { expectedStatus: 400 },
      );
      assertApiError(bad, 400, "bad_request", "PATCH counter invalid delta");
    });

    group("listing: formats and value projections", () => {
      // Each listing sub-test uses an isolated bucket to avoid test coupling and ordering assumptions.
      const listBucketBase = bucketSetupRestrictedAndSigned();

      group("listing: text format (values=true)", () => {
        const bidText = createRestrictedBucket({
          createdBuckets,
          flow: "list_text",
          form: listBucketBase,
        });
        putTextKey({
          bid: bidText,
          key: "k1",
          value: "hello",
          token: listBucketBase.write_key,
          flow: "list_text",
        });

        const res = listBucket(
          bidText,
          listBucketBase.read_key,
          "values=true&format=text",
          tags("list_text"),
        );
        checkRes(res, `GET /api/v1/${bidText}/?values=true&format=text`, () => {
          expect(res.status, "status").toBe(200);
          assertContentType(res, "text/plain; charset=utf-8", "list text");
          expect(res.body, "body").toBe("k1=hello");
        });
      });

      group("listing: json format (keys only)", () => {
        const bucketJson = bucketSetupRestrictedAndSigned();
        const bidJson = createRestrictedBucket({
          createdBuckets,
          flow: "list_json",
          form: bucketJson,
        });
        for (const k of ["c", "a", "b"]) {
          putTextKey({
            bid: bidJson,
            key: k,
            value: "1",
            token: bucketJson.write_key,
            flow: "list_json",
          });
        }
        const res = listBucket(
          bidJson,
          bucketJson.read_key,
          "format=json",
          tags("list_json"),
        );
        checkRes(res, "list json", () => {
          expect(res.status, "status").toBe(200);
          assertContentType(res, "application/json", "list json");
          assertJsonEquals(
            parseJson(res, "list json"),
            ["a", "b", "c"],
            "list json body",
          );
        });
      });

      group("listing: json format (values=true)", () => {
        const bucketJsonValues = bucketSetupRestrictedAndSigned();
        const bidJsonValues = createRestrictedBucket({
          createdBuckets,
          flow: "list_json_values",
          form: bucketJsonValues,
        });
        putJsonKey({
          bid: bidJsonValues,
          key: "jk",
          value: { a: 1 },
          token: bucketJsonValues.write_key,
          flow: "list_json_values",
        });

        const res = listBucket(
          bidJsonValues,
          bucketJsonValues.read_key,
          "values=true&format=json",
          tags("list_json_values"),
        );
        checkRes(res, "list json values", () => {
          expect(res.status, "status").toBe(200);
          assertContentType(res, "application/json", "list json values");
          assertJsonEquals(
            parseJson(res, "list json values"),
            [["jk", { a: 1 }]],
            "list json values body",
          );
        });
      });

      group("listing: jsonl format (values=true)", () => {
        const bucketJsonl = bucketSetupRestrictedAndSigned();
        const bidJsonl = createRestrictedBucket({
          createdBuckets,
          flow: "list_jsonl",
          form: bucketJsonl,
        });
        putTextKey({
          bid: bidJsonl,
          key: "x",
          value: "42",
          token: bucketJsonl.write_key,
          flow: "list_jsonl",
        });

        const res = listBucket(
          bidJsonl,
          bucketJsonl.read_key,
          "values=true&format=jsonl",
          tags("list_jsonl"),
        );
        checkRes(res, "list jsonl", () => {
          expect(res.status, "status").toBe(200);
          assertContentType(res, "application/x-ndjson", "list jsonl");
          expect(String(res.body).trim(), "body").toBe('["x",42]');
        });
      });

      group("listing: prefix filter", () => {
        const bucketPrefix = bucketSetupRestrictedAndSigned();
        const bidPrefix = createRestrictedBucket({
          createdBuckets,
          flow: "list_prefix",
          form: bucketPrefix,
        });
        for (const k of ["foo:1", "foo:2", "bar:1"]) {
          putTextKey({
            bid: bidPrefix,
            key: k,
            value: "x",
            token: bucketPrefix.write_key,
            flow: "list_prefix",
          });
        }

        const res = listBucket(
          bidPrefix,
          bucketPrefix.read_key,
          "format=json&prefix=foo%3A",
          tags("list_prefix"),
        );
        checkRes(res, "list prefix", () => {
          expect(res.status, "status").toBe(200);
          assertContentType(res, "application/json", "list prefix");
          assertJsonEquals(
            parseJson(res, "list prefix"),
            ["foo:1", "foo:2"],
            "list prefix body",
          );
        });
      });

      group("listing: reverse order", () => {
        const bucketReverse = bucketSetupRestrictedAndSigned();
        const bidReverse = createRestrictedBucket({
          createdBuckets,
          flow: "list_reverse",
          form: bucketReverse,
        });
        for (const k of ["a", "b", "c"]) {
          putTextKey({
            bid: bidReverse,
            key: k,
            value: "1",
            token: bucketReverse.write_key,
            flow: "list_reverse",
          });
        }

        const res = listBucket(
          bidReverse,
          bucketReverse.read_key,
          "format=json&reverse=true",
          tags("list_reverse"),
        );
        checkRes(res, "list reverse", () => {
          expect(res.status, "status").toBe(200);
          assertContentType(res, "application/json", "list reverse");
          assertJsonEquals(
            parseJson(res, "list reverse"),
            ["c", "b", "a"],
            "list reverse body",
          );
        });
      });

      group("listing: skip+limit pagination", () => {
        const bucketSkipLimit = bucketSetupRestrictedAndSigned();
        const bidSkipLimit = createRestrictedBucket({
          createdBuckets,
          flow: "list_skip_limit",
          form: bucketSkipLimit,
        });
        for (const k of ["k0", "k1", "k2", "k3"]) {
          putTextKey({
            bid: bidSkipLimit,
            key: k,
            value: "1",
            token: bucketSkipLimit.write_key,
            flow: "list_skip_limit",
          });
        }
        const res = listBucket(
          bidSkipLimit,
          bucketSkipLimit.read_key,
          "format=json&limit=2&skip=1",
          tags("list_skip_limit"),
        );
        checkRes(res, "list skip/limit", () => {
          expect(res.status, "status").toBe(200);
          assertContentType(res, "application/json", "list skip/limit");
          assertJsonEquals(
            parseJson(res, "list skip/limit"),
            ["k1", "k2"],
            "list skip/limit body",
          );
        });
      });

      group("listing: invalid format is 406 envelope", () => {
        // Invalid formats must use the standard JSON error envelope so clients can rely on it.
        const bucketBadFormat = bucketSetupRestrictedAndSigned();
        const bidBadFormat = createRestrictedBucket({
          createdBuckets,
          flow: "list_bad_format",
          form: bucketBadFormat,
        });
        const badFormat = listBucket(
          bidBadFormat,
          bucketBadFormat.read_key,
          "format=not-a-format",
          tags("list_bad_format"),
          { expectedStatus: 406 },
        );
        assertApiError(badFormat, 406, "not_acceptable", "list invalid format");
      });
    });

    group("txn: typed JSON vs plaintext inference", () => {
      const bucketTxn = bucketSetupRestrictedAndSigned();
      const bidTxn = createRestrictedBucket({
        createdBuckets,
        flow: "txn",
        form: bucketTxn,
      });

      group("txn: string value stays text/plain", () => {
        // In txn, *string* values are treated as raw bytes (no JSON content-type),
        // to preserve exact user payloads without semantic re-encoding.
        const txnS = runTransaction(
          bidTxn,
          bucketTxn.write_key,
          { txn: [{ set: "s", value: "olá" }] },
          tags("txn"),
        );
        checkRes(txnS, `POST /api/v1/${bidTxn}`, () => {
          expect(txnS.status, "status").toBe(204);
        });
        getTextKey({
          bid: bidTxn,
          key: "s",
          expectedBody: "olá",
          token: bucketTxn.read_key,
          flow: "txn",
        });
      });

      const jsonCases = [
        { key: "i", value: 42, expected: 42 },
        { key: "f", value: 1.5, expected: 1.5 },
        { key: "b", value: true, expected: true },
        { key: "a", value: [1, 2, 3], expected: [1, 2, 3] },
        { key: "o", value: { a: 1, b: "x" }, expected: { a: 1, b: "x" } },
      ];
      group("txn: non-string JSON values become application/json", () => {
        // In txn, non-string JSON values are encoded as JSON and served back with application/json.
        for (const c of jsonCases) {
          const txn = runTransaction(
            bidTxn,
            bucketTxn.write_key,
            { txn: [{ set: c.key, value: c.value }] },
            tags("txn"),
          );
          checkRes(txn, `txn set ${c.key}`, () => {
            expect(txn.status, "status").toBe(204);
          });
          getJsonKey({
            bid: bidTxn,
            key: c.key,
            expected: c.expected,
            token: bucketTxn.read_key,
            flow: "txn",
          });
        }
      });

      group("txn: numeric string stays text/plain (inference)", () => {
        // "42" is a string here, so it should *not* be served as JSON number 42.
        const txnNs = runTransaction(
          bidTxn,
          bucketTxn.write_key,
          { txn: [{ set: "ns", value: "42" }] },
          tags("txn"),
        );
        checkRes(txnNs, "txn numeric string", () => {
          expect(txnNs.status, "status").toBe(204);
        });
        getTextKey({
          bid: bidTxn,
          key: "ns",
          expectedBody: "42",
          token: bucketTxn.read_key,
          flow: "txn",
        });
      });

      const invalidPayloads = [
        { label: "null value", payload: { txn: [{ set: "k", value: null }] } },
        { label: "missing value", payload: { txn: [{ set: "k" }] } },
        {
          label: "both set and delete",
          payload: { txn: [{ set: "a", delete: "b", value: "v" }] },
        },
        {
          label: "unknown extra field",
          payload: { txn: [{ set: "k", value: "v", extra: true }] },
        },
      ];
      for (const c of invalidPayloads) {
        const res = runTransaction(
          bidTxn,
          bucketTxn.write_key,
          c.payload,
          tags("txn_invalid"),
          { expectedStatus: 400 },
        );
        assertApiError(res, 400, "bad_request", `txn invalid: ${c.label}`);
      }

      const resNoPartial = runTransaction(
        bidTxn,
        bucketTxn.write_key,
        {
          txn: [
            { set: "p1", value: "ok" },
            { set: "p2", value: null },
          ],
        },
        tags("txn_no_partial"),
        { expectedStatus: 400 },
      );
      assertApiError(
        resNoPartial,
        400,
        "bad_request",
        "txn no partial (null later)",
      );

      getKeyApiError({
        bid: bidTxn,
        key: "p1",
        token: bucketTxn.read_key,
        flow: "txn_no_partial",
        expectedStatus: 404,
        expectedCode: 404,
        expectedMessage: "not_found",
        ctx: "txn no partial: p1 absent",
      });
    });

    group("tokens: contract and error matrix", () => {
      // Tokens are security-critical; we validate both success shape and a small error matrix.
      const bucketTok = bucketSetupRestrictedAndSigned();
      const bidTok = createRestrictedBucket({
        createdBuckets,
        flow: "tokens",
        form: bucketTok,
      });

      mintTokenChecked({
        flow: "tokens",
        bid: bidTok,
        secretKey: bucketTok.secret_key,
        body: { prefix: "scope:", permissions: "read", ttl: "3600" },
      });

      const badTtl0 = mintToken(
        bidTok,
        bucketTok.secret_key,
        { prefix: "scope:", permissions: "read", ttl: "0" },
        tags("tokens"),
        { expectedStatus: 400 },
      );
      assertApiError(badTtl0, 400, "bad_request", "mint token ttl=0");

      const badTtlNeg = mintToken(
        bidTok,
        bucketTok.secret_key,
        { prefix: "scope:", permissions: "read", ttl: "-1" },
        tags("tokens"),
        { expectedStatus: 400 },
      );
      assertApiError(badTtlNeg, 400, "bad_request", "mint token ttl=-1");

      const badPrefix = mintToken(
        bidTok,
        bucketTok.secret_key,
        { prefix: "", permissions: "read", ttl: "3600" },
        tags("tokens"),
        { expectedStatus: 400 },
      );
      assertApiError(badPrefix, 400, "bad_request", "mint token empty prefix");

      const badPerm = mintToken(
        bidTok,
        bucketTok.secret_key,
        { prefix: "scope:", permissions: "nope", ttl: "3600" },
        tags("tokens"),
        { expectedStatus: 400 },
      );
      assertApiError(
        badPerm,
        400,
        "bad_request",
        "mint token invalid permission",
      );

      const noSign = {
        ...bucketSetupRestrictedAndSigned(),
        signing_key: "",
      };
      const bidNoSign = createRestrictedBucket({
        createdBuckets,
        flow: "tokens_no_sign",
        form: noSign,
      });
      const noSigning = mintToken(
        bidNoSign,
        noSign.secret_key,
        { prefix: "scope:", permissions: "read", ttl: "3600" },
        tags("tokens"),
        { expectedStatus: 503 },
      );
      // Without a signing key, the service must refuse minting (avoids producing unverifiable tokens).
      assertApiError(
        noSigning,
        503,
        "service_unavailable",
        "mint token without signing_key",
      );
    });
  } finally {
    if (!cleanupEnabled()) return;
    cleanupBuckets(createdBuckets);
  }
}
