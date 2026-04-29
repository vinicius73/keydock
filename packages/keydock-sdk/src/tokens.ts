import type { KyInstance } from "ky";

import { KeydockValidationError } from "./errors.js";
import { encodeBucketId } from "./internal/encoding.js";
import { writeRequestOptions } from "./internal/http.js";
import { normalizeKyError } from "./internal/response.js";
import type { AccessToken, CreateTokenInput, OperationOptions, TokenPermission } from "./types.js";

const VALID_PERMISSIONS = new Set<TokenPermission>(["read", "write", "enumerate", "delete"]);

type WireAccessToken = {
  access_token?: unknown;
};

export class TokensNamespace {
  constructor(
    private readonly bucketId: string,
    private readonly http: KyInstance,
  ) {}

  async create(input: CreateTokenInput, options?: OperationOptions): Promise<AccessToken> {
    try {
      const form = createTokenForm(input);
      const response = await this.http
        .post(`${encodeBucketId(this.bucketId)}/tokens/`, {
          ...writeRequestOptions(options),
          body: form,
          headers: {
            "Content-Type": "application/x-www-form-urlencoded",
          },
        })
        .json<WireAccessToken>();

      if (typeof response.access_token !== "string") {
        throw new KeydockValidationError("Invalid token response");
      }

      return {
        accessToken: response.access_token,
      };
    } catch (error) {
      if (error instanceof KeydockValidationError) {
        throw error;
      }

      throw await normalizeKyError(error);
    }
  }
}

function createTokenForm(input: CreateTokenInput): URLSearchParams {
  if (input.prefix.length === 0) {
    throw new KeydockValidationError("Token prefix must be non-empty");
  }
  if (
    !Number.isFinite(input.ttlSeconds) ||
    !Number.isInteger(input.ttlSeconds) ||
    input.ttlSeconds <= 0
  ) {
    throw new KeydockValidationError("ttlSeconds must be a positive integer");
  }
  if (input.permissions.length === 0) {
    throw new KeydockValidationError("Token permissions must be non-empty");
  }

  const permissions = new Set<TokenPermission>();
  for (const permission of input.permissions) {
    if (!VALID_PERMISSIONS.has(permission)) {
      throw new KeydockValidationError(`Invalid token permission: ${permission}`);
    }
    if (permissions.has(permission)) {
      throw new KeydockValidationError(`Duplicate token permission: ${permission}`);
    }
    permissions.add(permission);
  }

  const form = new URLSearchParams();
  form.set("prefix", input.prefix);
  form.set("permissions", [...permissions].join(","));
  form.set("ttl", input.ttlSeconds.toString());
  return form;
}
