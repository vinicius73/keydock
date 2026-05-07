import { HTTPError, TimeoutError } from "ky";

import {
  KeydockError,
  KeydockNetworkError,
  KeydockTimeoutError,
  KeydockValidationError,
} from "../errors.js";
import type { CounterValue } from "../types.js";

const INTEGER_COUNTER_RESPONSE = /^[+-]?\d+$/;
const MIN_SAFE_INTEGER_BIGINT = BigInt(Number.MIN_SAFE_INTEGER);
const MAX_SAFE_INTEGER_BIGINT = BigInt(Number.MAX_SAFE_INTEGER);

type ErrorEnvelope = {
  error?: {
    code?: unknown;
    message?: unknown;
  };
};

export function parseCounterResponse(raw: string): CounterValue {
  const value = raw.trim();
  if (INTEGER_COUNTER_RESPONSE.test(value)) {
    const bigint = BigInt(value);

    if (bigint >= MIN_SAFE_INTEGER_BIGINT && bigint <= MAX_SAFE_INTEGER_BIGINT) {
      return {
        raw: value,
        kind: "integer",
        bigint,
        number: Number(bigint),
      };
    }

    return {
      raw: value,
      kind: "integer",
      bigint,
    };
  }

  const number = Number(value);
  if (Number.isFinite(number)) {
    return {
      raw: value,
      kind: "float",
      number,
    };
  }

  throw new KeydockValidationError(`Invalid counter response: ${value}`);
}

export function isNotFoundError(error: unknown): boolean {
  if (error instanceof HTTPError) {
    return error.response.status === 404;
  }

  return error instanceof KeydockError && error.status === 404;
}

export async function normalizeOperationError(error: unknown): Promise<never> {
  if (error instanceof KeydockValidationError) {
    throw error;
  }

  throw await normalizeKyError(error);
}

export async function parseErrorBody(
  response: Response,
  request: Request | undefined,
  cause: unknown,
): Promise<KeydockError> {
  const fallbackDetail = response.statusText || "request_failed";
  const body = await readJsonEnvelope(response);
  const code = typeof body?.error?.code === "number" ? body.error.code : response.status;
  const detail = typeof body?.error?.message === "string" ? body.error.message : fallbackDetail;
  const input: ConstructorParameters<typeof KeydockError>[0] = {
    status: response.status,
    code,
    detail,
    response,
    cause,
  };

  if (request !== undefined) {
    input.request = request;
  }

  return new KeydockError(input);
}

export async function normalizeKyError(error: unknown): Promise<never> {
  if (error instanceof HTTPError) {
    throw await parseErrorBody(error.response, error.request, error);
  }

  if (error instanceof TimeoutError) {
    throw new KeydockTimeoutError({
      message: "Keydock request timed out",
      cause: error,
    });
  }

  throw new KeydockNetworkError({
    message: "Keydock network request failed",
    cause: error,
  });
}

async function readJsonEnvelope(response: Response): Promise<ErrorEnvelope | undefined> {
  try {
    return (await response.clone().json()) as ErrorEnvelope;
  } catch {
    return undefined;
  }
}
