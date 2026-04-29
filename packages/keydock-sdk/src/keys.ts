import type { KyInstance } from "ky";

import { KeydockError, KeydockValidationError } from "./errors.js";
import { encodeBucketId, encodeKey } from "./internal/encoding.js";
import { readRequestOptions, writeRequestOptions } from "./internal/http.js";
import { normalizeKyError, parseCounterResponse } from "./internal/response.js";
import type {
  CounterDelta,
  CounterValue,
  JsonValue,
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
    return await http.get(keyPath(bucketId, key), readRequestOptions(options)).text();
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
  return orNull(() => getText(http, bucketId, key, options));
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

export async function getJsonOrNull<T = unknown>(
  http: KyInstance,
  bucketId: string,
  key: string,
  options?: ReadOptions<T>,
): Promise<T | null> {
  return orNull(() => getJson<T>(http, bucketId, key, options));
}

export async function getBytes(
  http: KyInstance,
  bucketId: string,
  key: string,
  options?: OperationOptions,
): Promise<Uint8Array> {
  try {
    const response = await http.get(keyPath(bucketId, key), readRequestOptions(options));
    const responseWithBytes = response as Response & { bytes?: () => Promise<Uint8Array> };
    if (responseWithBytes.bytes !== undefined) {
      return responseWithBytes.bytes();
    }

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
  return orNull(() => getBytes(http, bucketId, key, options));
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
    const normalized = await catchNormalizedError(error);
    if (normalized instanceof KeydockError && normalized.status === 404) {
      return false;
    }

    throw normalized;
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

export function keyPath(bucketId: string, key: string): string {
  return `${encodeBucketId(bucketId)}/${encodeKey(key)}`;
}

export function pathWithTtl(path: string, ttlSeconds: number | undefined): string {
  if (ttlSeconds === undefined) {
    return path;
  }

  validateTtlSeconds(ttlSeconds);
  return `${path}?ttl=${ttlSeconds}`;
}

export function validateTtlSeconds(ttlSeconds: number): void {
  if (!Number.isFinite(ttlSeconds) || !Number.isInteger(ttlSeconds) || ttlSeconds < 0) {
    throw new KeydockValidationError(
      "ttlSeconds must be a finite integer greater than or equal to 0",
    );
  }
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
    | { body: BodyInit | Uint8Array | ArrayBuffer | Blob | string; contentType: string }
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
      body: toBodyInit(value.body),
      headers: {
        "Content-Type": value.contentType,
      },
    });
  } catch (error) {
    throw await normalizeOperationError(error);
  }
}

function toBodyInit(value: BodyInit | Uint8Array | ArrayBuffer | Blob | string): BodyInit {
  if (value instanceof Uint8Array) {
    return value.slice().buffer;
  }

  return value;
}

async function orNull<T>(operation: () => Promise<T>): Promise<T | null> {
  try {
    return await operation();
  } catch (error) {
    if (error instanceof KeydockError && error.status === 404) {
      return null;
    }

    throw error;
  }
}

async function catchNormalizedError(error: unknown): Promise<unknown> {
  try {
    await normalizeOperationError(error);
  } catch (normalized) {
    return normalized;
  }
}

async function normalizeOperationError(error: unknown): Promise<never> {
  if (error instanceof KeydockValidationError) {
    throw error;
  }

  throw await normalizeKyError(error);
}
