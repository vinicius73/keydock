import http from "k6/http";
import { check, fail } from "k6";

export function url(path) {
  const base = __ENV.KEYDOCK_BASE_URL;
  if (!base) throw new Error("missing required env var: KEYDOCK_BASE_URL");
  const trimmed = String(base).replace(/\/+$/, "");
  const p = String(path);
  return trimmed + (p.charAt(0) === "/" ? "" : "/") + p;
}

export function bearerHeaders(token) {
  return { Authorization: `Bearer ${token}` };
}

export function formEncode(fields) {
  const parts = [];
  for (const [k, v] of Object.entries(fields)) {
    if (v === undefined || v === null) continue;
    parts.push(`${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`);
  }
  return parts.join("&");
}

function redactAccessToken(body) {
  if (!body) return body;
  return body.replace(
    /\"access_token\"\\s*:\\s*\"[^\"]+\"/g,
    '"access_token":"[REDACTED]"',
  );
}

function failWithContext(ctx, res, opts = {}) {
  const body = opts.redactAccessToken ? redactAccessToken(res.body) : res.body;
  const snippet = body ? body.slice(0, 600) : "";
  fail(`${ctx} (status=${res.status}) body=${JSON.stringify(snippet)}`);
}

export function must(res, checks, ctx, opts) {
  const ok = check(res, checks);
  if (!ok) failWithContext(ctx, res, opts);
  return res;
}

function mergeHeaders(base, extra) {
  const out = {};
  if (base) {
    for (const k in base) out[k] = base[k];
  }
  if (extra) {
    for (const k in extra) out[k] = extra[k];
  }
  return out;
}

function mergeParams(params, headers) {
  const out = {};
  if (params) {
    for (const k in params) out[k] = params[k];
  }
  out.headers = headers;
  return out;
}

export function get(path, params) {
  return http.get(url(path), params);
}

export function del(path, params) {
  return http.del(url(path), null, params);
}

export function putText(path, text, params) {
  return http.put(url(path), text, params);
}

export function postText(path, text, params) {
  return http.post(url(path), text, params);
}

export function postForm(path, fields, params = {}) {
  const body = formEncode(fields);
  const headers = mergeHeaders(
    { "Content-Type": "application/x-www-form-urlencoded" },
    params.headers,
  );
  return http.post(url(path), body, mergeParams(params, headers));
}

export function postJson(path, obj, params = {}) {
  const body = JSON.stringify(obj);
  const headers = mergeHeaders(
    { "Content-Type": "application/json" },
    params.headers,
  );
  return http.post(url(path), body, mergeParams(params, headers));
}

export function patchJson(path, obj, params = {}) {
  const body = JSON.stringify(obj);
  const headers = mergeHeaders(
    { "Content-Type": "application/json" },
    params.headers,
  );
  return http.request("PATCH", url(path), body, mergeParams(params, headers));
}

export function parseJson(res, ctx, opts) {
  try {
    return res.json();
  } catch (_) {
    failWithContext(`${ctx}: invalid JSON`, res, opts);
  }
}
