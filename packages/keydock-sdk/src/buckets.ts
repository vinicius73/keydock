import type { KyInstance } from "ky";

import { KeydockError, KeydockValidationError } from "./errors.js";
import { encodeBucketId } from "./internal/encoding.js";
import { readRequestOptions, writeRequestOptions } from "./internal/http.js";
import { normalizeKyError } from "./internal/response.js";
import { validateTtlSeconds } from "./keys.js";
import type {
  BucketPolicy,
  CreateBucketInput,
  CreatedBucket,
  OperationOptions,
  UpdateBucketPolicyInput,
} from "./types.js";

type WireBucketPolicy = {
  default_ttl?: number;
  has_secret_key: boolean;
  has_read_key: boolean;
  has_write_key: boolean;
  has_signing_key: boolean;
  signing_key_generation: number;
  anonymous_access: {
    read: boolean;
    write: boolean;
    enumerate: boolean;
    delete: boolean;
  };
};

export class BucketsNamespace {
  constructor(private readonly http: KyInstance) {}

  async create(input: CreateBucketInput, options?: OperationOptions): Promise<CreatedBucket> {
    try {
      const form = createBucketForm(input);
      const id = await this.http
        .post("", {
          ...writeRequestOptions(options),
          body: form,
          headers: {
            "Content-Type": "application/x-www-form-urlencoded",
          },
        })
        .text();

      return { id };
    } catch (error) {
      throw await normalizeOperationError(error);
    }
  }

  async getPolicy(bucketId: string, options?: OperationOptions): Promise<BucketPolicy> {
    try {
      const policy = await this.http
        .get(encodeBucketId(bucketId), readRequestOptions(options))
        .json<WireBucketPolicy>();

      return fromWirePolicy(policy);
    } catch (error) {
      throw await normalizeOperationError(error);
    }
  }

  async updatePolicy(
    bucketId: string,
    patch: UpdateBucketPolicyInput,
    options?: OperationOptions,
  ): Promise<void> {
    try {
      const body = updatePolicyBody(patch);
      await this.http.patch(encodeBucketId(bucketId), {
        ...writeRequestOptions(options),
        json: body,
      });
    } catch (error) {
      throw await normalizeOperationError(error);
    }
  }

  async delete(bucketId: string, options?: OperationOptions): Promise<void> {
    try {
      await this.http.delete(`${encodeBucketId(bucketId)}/`, writeRequestOptions(options));
    } catch (error) {
      throw await normalizeOperationError(error);
    }
  }

  async exists(bucketId: string, options?: OperationOptions): Promise<boolean> {
    try {
      await this.http.head(encodeBucketId(bucketId), readRequestOptions(options));
      return true;
    } catch (error) {
      const normalized = await catchNormalizedError(error);
      if (normalized instanceof KeydockError && normalized.status === 404) {
        return false;
      }

      throw normalized;
    }
  }
}

function createBucketForm(input: CreateBucketInput): URLSearchParams {
  const form = new URLSearchParams();
  form.set("email", input.email);
  appendOptional(form, "secret_key", input.secretKey);
  appendOptional(form, "read_key", input.readKey);
  appendOptional(form, "write_key", input.writeKey);
  appendOptional(form, "signing_key", input.signingKey);

  if (input.defaultTtlSeconds !== undefined) {
    validateTtlSeconds(input.defaultTtlSeconds);
    form.set("default_ttl", input.defaultTtlSeconds.toString());
  }

  return form;
}

function updatePolicyBody(patch: UpdateBucketPolicyInput): Record<string, string | number | null> {
  if (patch.secretKey === null) {
    throw new KeydockValidationError("secretKey cannot be cleared");
  }

  const body: Record<string, string | number | null> = {};
  appendPatch(body, "secret_key", patch.secretKey);
  appendPatch(body, "read_key", patch.readKey);
  appendPatch(body, "write_key", patch.writeKey);
  appendPatch(body, "signing_key", patch.signingKey);

  if (patch.defaultTtlSeconds !== undefined) {
    if (patch.defaultTtlSeconds !== null) {
      validateTtlSeconds(patch.defaultTtlSeconds);
    }
    body.default_ttl = patch.defaultTtlSeconds;
  }

  return body;
}

function fromWirePolicy(policy: WireBucketPolicy): BucketPolicy {
  const result: BucketPolicy = {
    hasSecretKey: policy.has_secret_key,
    hasReadKey: policy.has_read_key,
    hasWriteKey: policy.has_write_key,
    hasSigningKey: policy.has_signing_key,
    signingKeyGeneration: policy.signing_key_generation,
    anonymousAccess: {
      read: policy.anonymous_access.read,
      write: policy.anonymous_access.write,
      enumerate: policy.anonymous_access.enumerate,
      delete: policy.anonymous_access.delete,
    },
  };

  if (policy.default_ttl !== undefined) {
    result.defaultTtlSeconds = policy.default_ttl;
  }

  return result;
}

function appendOptional(form: URLSearchParams, key: string, value: string | undefined): void {
  if (value !== undefined) {
    form.set(key, value);
  }
}

function appendPatch(
  body: Record<string, string | number | null>,
  key: string,
  value: string | null | undefined,
): void {
  if (value !== undefined) {
    body[key] = value;
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
