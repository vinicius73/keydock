import type { KyInstance, Options as KyOptions } from "ky";
import ky from "ky";

import type { KeydockOptions, OperationOptions } from "../types.js";
import { resolveAuth } from "./auth.js";
import { normalizeBaseUrl } from "./encoding.js";

const SAFE_RETRY: KyOptions["retry"] = {
  limit: 2,
  methods: ["get", "head"],
  statusCodes: [408, 429, 500, 502, 503, 504],
  afterStatusCodes: [429, 503],
  backoffLimit: 3000,
};

const NO_RETRY: KyOptions["retry"] = {
  limit: 0,
};

type BeforeRequestHook = NonNullable<NonNullable<KyOptions["hooks"]>["beforeRequest"]>[number];

export function buildKy(options: KeydockOptions): KyInstance {
  const base = options.http ?? ky;
  const requestDefaults = options.request ?? {};

  const mergedOptions = {
    timeout: 10_000,
    retry: SAFE_RETRY,
    ...requestDefaults,
    prefixUrl: normalizeBaseUrl(options.baseUrl),
    hooks: mergeBeforeRequestHook(requestDefaults, async (request: Request) => {
      const credential = await resolveAuth(options.auth);
      if (credential !== undefined) {
        request.headers.set("Authorization", `Bearer ${credential}`);
      }
    }),
  } as KyOptions;

  return base.extend(mergedOptions);
}

export function readRequestOptions(options: OperationOptions | undefined): KyOptions | undefined {
  return options?.request;
}

export function writeRequestOptions(options: OperationOptions | undefined): KyOptions {
  return {
    ...options?.request,
    retry: NO_RETRY,
  } as KyOptions;
}

function mergeBeforeRequestHook(
  requestDefaults: KyOptions,
  beforeRequest: BeforeRequestHook,
): KyOptions["hooks"] {
  const hooks = requestDefaults.hooks ?? {};
  return {
    ...hooks,
    beforeRequest: [...(hooks.beforeRequest ?? []), beforeRequest],
  };
}
