import type { KyInstance } from "ky";

import { KeydockValidationError } from "./errors.js";
import { encodeBucketId, encodeKey } from "./internal/encoding.js";
import { writeRequestOptions } from "./internal/http.js";
import { normalizeKyError } from "./internal/response.js";
import { validateTtlSeconds } from "./keys.js";
import type { OperationOptions, TransactionOperation } from "./types.js";

type WireTxnItem =
  | {
      set: string;
      value: unknown;
      ttl?: number;
    }
  | {
      delete: string;
    };

export async function executeTransaction(
  http: KyInstance,
  bucketId: string,
  operations: readonly TransactionOperation[],
  options?: OperationOptions,
): Promise<void> {
  const txn = serializeTransaction(operations);

  try {
    await http.post(encodeBucketId(bucketId), {
      ...writeRequestOptions(options),
      json: { txn },
    });
  } catch (error) {
    if (error instanceof KeydockValidationError) {
      throw error;
    }

    throw await normalizeKyError(error);
  }
}

export function serializeTransaction(operations: readonly TransactionOperation[]): WireTxnItem[] {
  if (operations.length === 0) {
    throw new KeydockValidationError("Transaction must contain at least one operation");
  }

  return operations.map((operation) => {
    if ("delete" in operation) {
      return {
        delete: encodeKey(operation.delete),
      };
    }

    if (operation.value === null) {
      throw new KeydockValidationError("Transaction set values must not be null");
    }

    const item: WireTxnItem = {
      set: encodeKey(operation.set),
      value: operation.value,
    };

    if (operation.ttlSeconds !== undefined) {
      validateTtlSeconds(operation.ttlSeconds);
      item.ttl = operation.ttlSeconds;
    }

    return item;
  });
}
