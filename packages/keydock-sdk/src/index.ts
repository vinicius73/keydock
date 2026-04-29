export { BucketHandle } from "./bucket.js";
export { BucketsNamespace } from "./buckets.js";
export type { KeydockClient } from "./client.js";
export { createKeydock } from "./client.js";

export {
  KeydockError,
  KeydockNetworkError,
  KeydockTimeoutError,
  KeydockValidationError,
} from "./errors.js";
export { TokensNamespace } from "./tokens.js";

export type {
  AccessToken,
  AnonymousAccess,
  BucketPolicy,
  CounterDelta,
  CounterValue,
  CreateBucketInput,
  CreatedBucket,
  CreateTokenInput,
  JsonArray,
  JsonObject,
  JsonPrimitive,
  JsonValue,
  KeydockAuth,
  KeydockListEntry,
  KeydockOptions,
  ListEntriesOptions,
  ListKeysOptions,
  OperationOptions,
  ReadOptions,
  TokenPermission,
  TransactionOperation,
  UpdateBucketPolicyInput,
  WriteOptions,
} from "./types.js";
