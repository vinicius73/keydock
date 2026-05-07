import http from "k6/http";

import {
  bearerHeaders,
  del,
  get,
  head,
  patchJson,
  patchText,
  postForm,
  postJson,
  postText,
  putBytes,
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

export function postReady(tags, options) {
  return postText(
    "/ready",
    "",
    params("POST /ready", tags, requestOptions(options)),
  );
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

export function putKey(bucketId, key, value, token, tags, options, extra) {
  return putText(
    `/api/v1/${bucketId}/${key}`,
    value,
    authedParams(
      token,
      "PUT /api/v1/:bucket/:key",
      tags,
      mergeObjects(requestOptions(options), extra),
    ),
  );
}

export function postKey(bucketId, key, value, token, tags, options, extra) {
  return postText(
    `/api/v1/${bucketId}/${key}`,
    value,
    authedParams(
      token,
      "POST /api/v1/:bucket/:key",
      tags,
      mergeObjects(requestOptions(options), extra),
    ),
  );
}

export function putKeyJson(bucketId, key, jsonText, token, tags, options, extra) {
  const headers = mergeObjects(
    { "Content-Type": "application/json" },
    extra && extra.headers,
  );
  const extraParams = extra ? mergeObjects(extra, { headers }) : { headers };
  return putText(
    `/api/v1/${bucketId}/${key}`,
    jsonText,
    authedParams(
      token,
      "PUT /api/v1/:bucket/:key",
      tags,
      mergeObjects(requestOptions(options), extraParams),
    ),
  );
}

export function putKeyBytes(bucketId, key, bytes, token, tags, options, extra) {
  return putBytes(
    `/api/v1/${bucketId}/${key}`,
    bytes,
    authedParams(
      token,
      "PUT /api/v1/:bucket/:key",
      tags,
      mergeObjects(requestOptions(options), extra),
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

export function getKey(bucketId, key, token, tags, options, extra) {
  return get(
    `/api/v1/${bucketId}/${key}`,
    authedParams(
      token,
      "GET /api/v1/:bucket/:key",
      tags,
      mergeObjects(requestOptions(options), extra),
    ),
  );
}

export function getKeyWithoutAuth(bucketId, key, tags, options) {
  return get(
    `/api/v1/${bucketId}/${key}`,
    params("GET /api/v1/:bucket/:key", tags, requestOptions(options)),
  );
}

export function headKey(bucketId, key, token, tags, options, extra) {
  return head(
    `/api/v1/${bucketId}/${key}`,
    authedParams(
      token,
      "HEAD /api/v1/:bucket/:key",
      tags,
      mergeObjects(requestOptions(options), extra),
    ),
  );
}

export function deleteKey(bucketId, key, token, tags) {
  return del(
    `/api/v1/${bucketId}/${key}`,
    authedParams(token, "DELETE /api/v1/:bucket/:key", tags),
  );
}

function withQuery(path, query) {
  if (!query) return path;
  return query.charAt(0) === "?" ? `${path}${query}` : `${path}?${query}`;
}

export function listBucket(bucketId, token, query, tags, options, extra) {
  return get(
    withQuery(`/api/v1/${bucketId}/`, query),
    authedParams(
      token,
      "GET /api/v1/:bucket/",
      tags,
      mergeObjects(requestOptions(options), extra),
    ),
  );
}

export function listKeysJson(bucketId, readKey, tags, options) {
  return get(
    `/api/v1/${bucketId}/`,
    authedParams(
      readKey,
      "GET /api/v1/:bucket/",
      tags,
      mergeObjects(requestOptions(options), {
        headers: { Accept: "application/json" },
      }),
    ),
  );
}

export function mintToken(bucketId, secretKey, body, tags, options, extra) {
  return postForm(
    `/api/v1/${bucketId}/tokens/`,
    body,
    authedParams(
      secretKey,
      "POST /api/v1/:bucket/tokens/",
      tags,
      mergeObjects(requestOptions(options), extra),
    ),
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

export function patchBucket(bucketId, secretKey, body, tags, options, extra) {
  return patchJson(
    `/api/v1/${bucketId}`,
    body,
    authedParams(
      secretKey,
      "PATCH /api/v1/:bucket",
      tags,
      mergeObjects(requestOptions(options), extra),
    ),
  );
}

export function patchKey(bucketId, key, deltaText, token, tags, options, extra) {
  return patchText(
    `/api/v1/${bucketId}/${key}`,
    deltaText,
    authedParams(
      token,
      "PATCH /api/v1/:bucket/:key",
      tags,
      mergeObjects(requestOptions(options), extra),
    ),
  );
}

export function getBucketPolicy(bucketId, secretKey, tags, options, extra) {
  return get(
    `/api/v1/${bucketId}`,
    authedParams(
      secretKey,
      "GET /api/v1/:bucket",
      tags,
      mergeObjects(requestOptions(options), extra),
    ),
  );
}

export function headBucket(bucketId, secretKey, tags, options, extra) {
  return head(
    `/api/v1/${bucketId}`,
    authedParams(
      secretKey,
      "HEAD /api/v1/:bucket",
      tags,
      mergeObjects(requestOptions(options), extra),
    ),
  );
}
