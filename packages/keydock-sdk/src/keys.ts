import type { KyInstance } from "ky";

import { KeydockValidationError } from "./errors.js";
import { encodeBucketId, encodeKey } from "./internal/encoding.js";
import { readRequestOptions, writeRequestOptions } from "./internal/http.js";
import {
  isNotFoundError,
  normalizeOperationError,
  parseCounterResponse,
} from "./internal/response.js";
import {
  validateNonNegativeInteger,
  validateTtlSeconds,
} from "./internal/validation.js";
import type {
  CounterDelta,
  CounterValue,
  JsonValue,
  KeydockListEntry,
  ListEntriesOptions,
  ListKeysOptions,
  OperationOptions,
  ReadOptions,
  WriteOptions,
} from "./types.js";

export async function getText(
  http: KyInstance,
  bucketId: string,
  key: string,
  options?: OperationOptions,
): Promise<string> {
  try {
    return await http
      .get(keyPath(bucketId, key), readRequestOptions(options))
      .text();
  } catch (error) {
    throw await normalizeOperationError(error);
  }
}

export async function getTextOrNull(
  http: KyInstance,
  bucketId: string,
  key: string,
  options?: OperationOptions,
): Promise<string | null> {
  return orNull(() =>
    http.get(keyPath(bucketId, key), readRequestOptions(options)).text(),
  );
}

export async function getJson<T = unknown>(
  http: KyInstance,
  bucketId: string,
  key: string,
  options?: ReadOptions<T>,
): Promise<T> {
  try {
    const value = await http
      .get(keyPath(bucketId, key), readRequestOptions(options))
      .json<unknown>();
    return options?.parse === undefined ? (value as T) : options.parse(value);
  } catch (error) {
    throw await normalizeOperationError(error);
  }
}

/**
 * Reads JSON and distinguishes a missing key from a stored JSON null value.
 *
 * Returns `undefined` only when the backend returns 404. A present key whose
 * JSON payload is `null` resolves to `null`.
 */
export async function getJsonOrNull<T = unknown>(
  http: KyInstance,
  bucketId: string,
  key: string,
  options?: ReadOptions<T>,
): Promise<T | null | undefined> {
  return orUndefined(async () => {
    const value = await http
      .get(keyPath(bucketId, key), readRequestOptions(options))
      .json<unknown>();
    return options?.parse === undefined ? (value as T) : options.parse(value);
  });
}

export async function getBytes(
  http: KyInstance,
  bucketId: string,
  key: string,
  options?: OperationOptions,
): Promise<Uint8Array> {
  try {
    const response = await http.get(
      keyPath(bucketId, key),
      readRequestOptions(options),
    );
    return new Uint8Array(await response.arrayBuffer());
  } catch (error) {
    throw await normalizeOperationError(error);
  }
}

export async function getBytesOrNull(
  http: KyInstance,
  bucketId: string,
  key: string,
  options?: OperationOptions,
): Promise<Uint8Array | null> {
  return orNull(async () => {
    const response = await http.get(
      keyPath(bucketId, key),
      readRequestOptions(options),
    );
    return new Uint8Array(await response.arrayBuffer());
  });
}

export async function setText(
  http: KyInstance,
  bucketId: string,
  key: string,
  value: string,
  options?: WriteOptions,
): Promise<void> {
  await writeValue(http, bucketId, key, options, {
    body: value,
    contentType: "text/plain; charset=utf-8",
  });
}

export async function setJson(
  http: KyInstance,
  bucketId: string,
  key: string,
  value: JsonValue,
  options?: WriteOptions,
): Promise<void> {
  await writeValue(http, bucketId, key, options, {
    json: value,
  });
}

export async function setBytes(
  http: KyInstance,
  bucketId: string,
  key: string,
  value: BodyInit | Uint8Array | ArrayBuffer | Blob,
  options?: WriteOptions,
): Promise<void> {
  await writeValue(http, bucketId, key, options, {
    body: value,
    contentType: "application/octet-stream",
  });
}

export async function deleteKey(
  http: KyInstance,
  bucketId: string,
  key: string,
  options?: OperationOptions,
): Promise<void> {
  try {
    await http.delete(keyPath(bucketId, key), writeRequestOptions(options));
  } catch (error) {
    throw await normalizeOperationError(error);
  }
}

export async function keyExists(
  http: KyInstance,
  bucketId: string,
  key: string,
  options?: OperationOptions,
): Promise<boolean> {
  try {
    await http.head(keyPath(bucketId, key), readRequestOptions(options));
    return true;
  } catch (error) {
    if (isNotFoundError(error)) {
      return false;
    }

    throw await normalizeOperationError(error);
  }
}

export async function increment(
  http: KyInstance,
  bucketId: string,
  key: string,
  delta: CounterDelta,
  options?: WriteOptions,
): Promise<CounterValue> {
  try {
    const response = await http
      .patch(pathWithTtl(keyPath(bucketId, key), options?.ttlSeconds), {
        ...writeRequestOptions(options),
        body: serializeDelta(delta),
        headers: {
          "Content-Type": "text/plain; charset=utf-8",
        },
      })
      .text();

    return parseCounterResponse(response);
  } catch (error) {
    throw await normalizeOperationError(error);
  }
}

export async function listKeys(
  http: KyInstance,
  bucketId: string,
  options?: ListKeysOptions,
): Promise<string[]> {
  try {
    const value = await http
      .get(listPath(bucketId), {
        ...readRequestOptions(options),
        searchParams: listSearchParams(options, false),
      })
      .json<unknown>();

    if (
      !Array.isArray(value) ||
      !value.every((item) => typeof item === "string")
    ) {
      throw new KeydockValidationError("Invalid listKeys response");
    }

    return value;
  } catch (error) {
    throw await normalizeOperationError(error);
  }
}

export async function listEntries(
  http: KyInstance,
  bucketId: string,
  options?: ListEntriesOptions,
): Promise<KeydockListEntry[]> {
  try {
    const value = await http
      .get(listPath(bucketId), {
        ...readRequestOptions(options),
        searchParams: listSearchParams(options, true),
      })
      .json<unknown>();

    if (!Array.isArray(value)) {
      throw new KeydockValidationError("Invalid listEntries response");
    }

    return value.map((entry) => {
      if (
        !Array.isArray(entry) ||
        entry.length !== 2 ||
        typeof entry[0] !== "string"
      ) {
        throw new KeydockValidationError("Invalid listEntries response");
      }

      return {
        key: entry[0],
        value: entry[1],
      };
    });
  } catch (error) {
    throw await normalizeOperationError(error);
  }
}

export function keyPath(bucketId: string, key: string): string {
  return `${encodeBucketId(bucketId)}/${encodeKey(key)}`;
}

export function listPath(bucketId: string): string {
  return `${encodeBucketId(bucketId)}/`;
}

export function pathWithTtl(
  path: string,
  ttlSeconds: number | undefined,
): string {
  if (ttlSeconds === undefined) {
    return path;
  }

  validateTtlSeconds(ttlSeconds);
  return `${path}?ttl=${ttlSeconds}`;
}

function listSearchParams(
  options: ListKeysOptions | undefined,
  includeValues: boolean,
): URLSearchParams {
  const searchParams = new URLSearchParams();
  searchParams.set("format", "json");
  searchParams.set("values", includeValues ? "true" : "false");

  if (options?.prefix !== undefined) {
    searchParams.set("prefix", options.prefix);
  }
  if (options?.limit !== undefined) {
    validateNonNegativeInteger("limit", options.limit);
    searchParams.set("limit", options.limit.toString());
  }
  if (options?.skip !== undefined) {
    validateNonNegativeInteger("skip", options.skip);
    searchParams.set("skip", options.skip.toString());
  }
  if (options?.reverse !== undefined) {
    searchParams.set("reverse", options.reverse ? "true" : "false");
  }

  return searchParams;
}

export function serializeDelta(delta: CounterDelta): string {
  if (typeof delta === "bigint") {
    if (delta === 0n) {
      throw new KeydockValidationError("Counter delta must not be 0");
    }

    return delta > 0n ? `+${delta.toString()}` : delta.toString();
  }

  if (!Number.isFinite(delta)) {
    throw new KeydockValidationError("Counter delta must be finite");
  }
  if (delta === 0) {
    throw new KeydockValidationError("Counter delta must not be 0");
  }
  if (Number.isInteger(delta) && !Number.isSafeInteger(delta)) {
    throw new KeydockValidationError(
      "Integer counter deltas must be safe integers; use bigint instead",
    );
  }

  return delta > 0 ? `+${delta}` : delta.toString();
}

async function writeValue(
  http: KyInstance,
  bucketId: string,
  key: string,
  options: WriteOptions | undefined,
  value:
    | { body: BodyInit | Uint8Array; contentType: string }
    | { json: JsonValue },
): Promise<void> {
  try {
    const requestOptions = writeRequestOptions(options);
    if ("json" in value) {
      await http.put(pathWithTtl(keyPath(bucketId, key), options?.ttlSeconds), {
        ...requestOptions,
        json: value.json,
      });
      return;
    }

    await http.put(pathWithTtl(keyPath(bucketId, key), options?.ttlSeconds), {
      ...requestOptions,
      body: toUploadBody(value.body),
      headers: {
        "Content-Type": value.contentType,
      },
    });
  } catch (error) {
    throw await normalizeOperationError(error);
  }
}

function toUploadBody(value: BodyInit | Uint8Array): BodyInit {
  if (!(value instanceof Uint8Array)) {
    return value;
  }

  if (value.buffer instanceof ArrayBuffer) {
    if (
      value.byteOffset === 0 &&
      value.byteLength === value.buffer.byteLength
    ) {
      return value.buffer;
    }

    return value.buffer.slice(
      value.byteOffset,
      value.byteOffset + value.byteLength,
    );
  }

  const copy = new Uint8Array(value.byteLength);
  copy.set(value);
  return copy.buffer;
}

async function orNull<T>(operation: () => Promise<T>): Promise<T | null> {
  try {
    return await operation();
  } catch (error) {
    if (isNotFoundError(error)) {
      return null;
    }

    throw await normalizeOperationError(error);
  }
}

async function orUndefined<T>(
  operation: () => Promise<T>,
): Promise<T | undefined> {
  try {
    return await operation();
  } catch (error) {
    if (isNotFoundError(error)) {
      return undefined;
    }

    throw await normalizeOperationError(error);
  }
}
