import type { KyInstance, Options as KyOptions } from "ky";
import ky from "ky";

import type { KeydockOptions, OperationOptions } from "../types.js";
import { resolveAuth } from "./auth.js";
import { normalizeBaseUrl } from "./encoding.js";

const SAFE_RETRY: NonNullable<KyOptions["retry"]> = {
  limit: 2,
  methods: ["get", "head"],
  statusCodes: [408, 429, 500, 502, 503, 504],
  afterStatusCodes: [429, 503],
  backoffLimit: 3000,
};

const NO_RETRY: NonNullable<KyOptions["retry"]> = {
  limit: 0,
};

type BeforeRequestHook = NonNullable<NonNullable<KyOptions["hooks"]>["beforeRequest"]>[number];

export function buildKy(options: KeydockOptions): KyInstance {
  const base = options.http ?? ky;
  const requestDefaults = options.request ?? {};

  const mergedOptions = {
    timeout: 10_000,
    ...requestDefaults,
    retry: requestDefaults.retry ?? SAFE_RETRY,
    prefix: normalizeBaseUrl(options.baseUrl),
    hooks: mergeBeforeRequestHook(requestDefaults, async ({ request }) => {
      const credential = await resolveAuth(options.auth);
      if (credential !== undefined) {
        request.headers.set("Authorization", `Bearer ${credential}`);
      }
    }),
  } satisfies KyOptions;

  return base.extend(mergedOptions);
}

export function readRequestOptions(options: OperationOptions | undefined): KyOptions | undefined {
  return options?.request;
}

export function writeRequestOptions(options: OperationOptions | undefined): KyOptions {
  const requestOptions = {
    ...options?.request,
    retry: NO_RETRY,
  } satisfies KyOptions;

  return requestOptions;
}

function mergeBeforeRequestHook(
  requestDefaults: KyOptions,
  beforeRequest: BeforeRequestHook,
): NonNullable<KyOptions["hooks"]> {
  const hooks = requestDefaults.hooks ?? {};
  return {
    ...hooks,
    beforeRequest: [...(hooks.beforeRequest ?? []), beforeRequest],
  };
}
