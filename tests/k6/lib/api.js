import http from "k6/http";

import {
  bearerHeaders,
  del,
  get,
  patchJson,
  postForm,
  postJson,
  putText,
} from "./client.js";

function mergeObjects(left, right) {
  const out = {};
  if (left) {
    for (const key in left) out[key] = left[key];
  }
  if (right) {
    for (const key in right) out[key] = right[key];
  }
  return out;
}

function params(name, tags = {}, extra = {}) {
  return mergeObjects(extra, {
    tags: mergeObjects(extra.tags, mergeObjects({ name: name }, tags)),
  });
}

function authedParams(token, name, tags = {}, extra = {}) {
  return params(
    name,
    tags,
    mergeObjects(extra, {
      headers: mergeObjects(extra.headers, bearerHeaders(token)),
    }),
  );
}

function requestOptions(options) {
  if (!options || options.expectedStatus === undefined) return {};
  return { responseCallback: http.expectedStatuses(options.expectedStatus) };
}

export function getReady(tags) {
  return get("/ready", params("GET /ready", tags));
}

export function scrapeMetrics(tags) {
  return get("/metrics", params("GET /metrics", tags));
}

export function createBucket(bucket, tags) {
  return postForm("/api/v1", bucket, params("POST /api/v1", tags));
}

export function deleteBucket(bucketId, secretKey, tags) {
  return del(
    `/api/v1/${bucketId}`,
    authedParams(secretKey, "DELETE /api/v1/:bucket", tags),
  );
}

export function putKey(bucketId, key, value, token, tags, options) {
  return putText(
    `/api/v1/${bucketId}/${key}`,
    value,
    authedParams(
      token,
      "PUT /api/v1/:bucket/:key",
      tags,
      requestOptions(options),
    ),
  );
}

export function putKeyWithoutAuth(bucketId, key, value, tags, options) {
  return putText(
    `/api/v1/${bucketId}/${key}`,
    value,
    params("PUT /api/v1/:bucket/:key", tags, requestOptions(options)),
  );
}

export function getKey(bucketId, key, token, tags, options) {
  return get(
    `/api/v1/${bucketId}/${key}`,
    authedParams(
      token,
      "GET /api/v1/:bucket/:key",
      tags,
      requestOptions(options),
    ),
  );
}

export function getKeyWithoutAuth(bucketId, key, tags, options) {
  return get(
    `/api/v1/${bucketId}/${key}`,
    params("GET /api/v1/:bucket/:key", tags, requestOptions(options)),
  );
}

export function deleteKey(bucketId, key, token, tags) {
  return del(
    `/api/v1/${bucketId}/${key}`,
    authedParams(token, "DELETE /api/v1/:bucket/:key", tags),
  );
}

export function listKeysJson(bucketId, readKey, tags) {
  return get(
    `/api/v1/${bucketId}/`,
    authedParams(readKey, "GET /api/v1/:bucket/", tags, {
      headers: { Accept: "application/json" },
    }),
  );
}

export function mintToken(bucketId, secretKey, body, tags) {
  return postForm(
    `/api/v1/${bucketId}/tokens/`,
    body,
    authedParams(secretKey, "POST /api/v1/:bucket/tokens/", tags),
  );
}

export function runTransaction(bucketId, secretKey, body, tags, options) {
  return postJson(
    `/api/v1/${bucketId}`,
    body,
    authedParams(
      secretKey,
      "POST /api/v1/:bucket",
      tags,
      requestOptions(options),
    ),
  );
}

export function patchBucket(bucketId, secretKey, body, tags) {
  return patchJson(
    `/api/v1/${bucketId}`,
    body,
    authedParams(secretKey, "PATCH /api/v1/:bucket", tags),
  );
}
