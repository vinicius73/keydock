import type { KyInstance, Options as KyOptions } from "ky";

export type JsonPrimitive = string | number | boolean | null;
export type JsonObject = { readonly [key: string]: JsonValue };
export type JsonArray = readonly JsonValue[];
export type JsonValue = JsonPrimitive | JsonObject | JsonArray;

export type KeydockAuth = string | (() => string | Promise<string>);

export type KeydockOptions = {
  baseUrl: string | URL;
  auth?: KeydockAuth;
  http?: KyInstance;
  request?: KyOptions;
};

export type OperationOptions = {
  request?: KyOptions;
};

export type WriteOptions = OperationOptions & {
  ttlSeconds?: number;
};

export type ReadOptions<T> = OperationOptions & {
  parse?: (value: unknown) => T;
};

export type CounterDelta = number | bigint;

export type CounterValue =
  | {
      raw: string;
      kind: "integer";
      bigint: bigint;
      number?: number;
    }
  | {
      raw: string;
      kind: "float";
      number: number;
    };

export type CreatedBucket = {
  id: string;
};

export type AnonymousAccess = {
  read: boolean;
  write: boolean;
  enumerate: boolean;
  delete: boolean;
};

export type BucketPolicy = {
  defaultTtlSeconds?: number;
  hasSecretKey: boolean;
  hasReadKey: boolean;
  hasWriteKey: boolean;
  hasSigningKey: boolean;
  signingKeyGeneration: number;
  anonymousAccess: AnonymousAccess;
};

export type UpdateBucketPolicyInput = {
  secretKey?: string | null;
  readKey?: string | null;
  writeKey?: string | null;
  signingKey?: string | null;
  defaultTtlSeconds?: number | null;
};

export type CreateBucketInput = {
  email: string;
  secretKey?: string;
  readKey?: string;
  writeKey?: string;
  signingKey?: string;
  defaultTtlSeconds?: number;
};

export type TokenPermission = "read" | "write" | "enumerate" | "delete";

export type CreateTokenInput = {
  prefix: string;
  permissions: TokenPermission[];
  ttlSeconds: number;
};

export type AccessToken = {
  accessToken: string;
};

export type ListKeysOptions = OperationOptions & {
  prefix?: string;
  limit?: number;
  skip?: number;
  reverse?: boolean;
};

export type ListEntriesOptions = ListKeysOptions;

export type KeydockListEntry = {
  key: string;
  value: unknown;
};

export type TransactionOperation =
  | { set: string; value: JsonValue; ttlSeconds?: number }
  | { delete: string };
