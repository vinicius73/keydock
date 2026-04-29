import type { KyInstance } from "ky";

import {
  deleteKey,
  getBytes,
  getBytesOrNull,
  getJson,
  getJsonOrNull,
  getText,
  getTextOrNull,
  increment,
  keyExists,
  listEntries,
  listKeys,
  setBytes,
  setJson,
  setText,
} from "./keys.js";
import { executeTransaction } from "./transactions.js";
import type {
  CounterDelta,
  CounterValue,
  JsonValue,
  KeydockListEntry,
  ListEntriesOptions,
  ListKeysOptions,
  OperationOptions,
  ReadOptions,
  TransactionOperation,
  WriteOptions,
} from "./types.js";

export class BucketHandle {
  constructor(
    private readonly bucketId: string,
    private readonly http: KyInstance,
  ) {}

  getText(key: string, options?: OperationOptions): Promise<string> {
    return getText(this.http, this.bucketId, key, options);
  }

  getTextOrNull(key: string, options?: OperationOptions): Promise<string | null> {
    return getTextOrNull(this.http, this.bucketId, key, options);
  }

  getJson<T = unknown>(key: string, options?: ReadOptions<T>): Promise<T> {
    return getJson<T>(this.http, this.bucketId, key, options);
  }

  getJsonOrNull<T = unknown>(key: string, options?: ReadOptions<T>): Promise<T | null> {
    return getJsonOrNull<T>(this.http, this.bucketId, key, options);
  }

  getBytes(key: string, options?: OperationOptions): Promise<Uint8Array> {
    return getBytes(this.http, this.bucketId, key, options);
  }

  getBytesOrNull(key: string, options?: OperationOptions): Promise<Uint8Array | null> {
    return getBytesOrNull(this.http, this.bucketId, key, options);
  }

  setText(key: string, value: string, options?: WriteOptions): Promise<void> {
    return setText(this.http, this.bucketId, key, value, options);
  }

  setJson(key: string, value: JsonValue, options?: WriteOptions): Promise<void> {
    return setJson(this.http, this.bucketId, key, value, options);
  }

  setBytes(
    key: string,
    value: BodyInit | Uint8Array | ArrayBuffer | Blob,
    options?: WriteOptions,
  ): Promise<void> {
    return setBytes(this.http, this.bucketId, key, value, options);
  }

  delete(key: string, options?: OperationOptions): Promise<void> {
    return deleteKey(this.http, this.bucketId, key, options);
  }

  exists(key: string, options?: OperationOptions): Promise<boolean> {
    return keyExists(this.http, this.bucketId, key, options);
  }

  increment(key: string, delta: CounterDelta, options?: WriteOptions): Promise<CounterValue> {
    return increment(this.http, this.bucketId, key, delta, options);
  }

  listKeys(options?: ListKeysOptions): Promise<string[]> {
    return listKeys(this.http, this.bucketId, options);
  }

  listEntries(options?: ListEntriesOptions): Promise<KeydockListEntry[]> {
    return listEntries(this.http, this.bucketId, options);
  }

  transaction(
    operations: readonly TransactionOperation[],
    options?: OperationOptions,
  ): Promise<void> {
    return executeTransaction(this.http, this.bucketId, operations, options);
  }
}
