import { KeydockValidationError } from "../errors.js";

const API_PREFIX = "/api/v1";

export function normalizeBaseUrl(baseUrl: string | URL): string {
  const url = new URL(baseUrl);
  const pathname = url.pathname.replace(/\/+$/, "");

  if (pathname === "" || pathname === "/") {
    url.pathname = `${API_PREFIX}/`;
  } else if (pathname === API_PREFIX) {
    url.pathname = `${API_PREFIX}/`;
  } else {
    url.pathname = `${pathname}${API_PREFIX}/`;
  }

  url.search = "";
  url.hash = "";

  return url.toString();
}

export function encodeKey(key: string): string {
  return encodeNonEmptySegment(key, "Key");
}

export function encodeBucketId(bucketId: string): string {
  return encodeNonEmptySegment(bucketId, "Bucket id");
}

function encodeNonEmptySegment(value: string, label: string): string {
  if (value.length === 0) {
    throw new KeydockValidationError(`${label} must be non-empty`);
  }

  return encodeURIComponent(value);
}
