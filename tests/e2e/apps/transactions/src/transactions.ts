import { createKeydock, KeydockError, KeydockValidationError } from "keydock-sdk";

import {
  readConfig,
  requireAuth,
  requireBucketId,
  requireKey,
} from "../../../src/browser-config.js";
import {
  appendLog,
  mountE2eApp,
  renderError,
  setAttr,
  setStatus,
  setStep,
  setText,
} from "../../../src/ui.js";

mountE2eApp({
  title: "Transaction Timeline",
  description:
    "The SDK posts a transaction from the browser and then verifies the resulting server state.",
  steps: [
    { id: "transaction-result", label: "Transaction" },
    { id: "first-read-result", label: "First Read" },
    { id: "second-read-result", label: "Second Read" },
    { id: "deleted-read-result", label: "Deleted Read" },
    { id: "counter-result", label: "Counter" },
    { id: "txn-json-integer", label: "JSON Integer" },
    { id: "txn-json-boolean", label: "JSON Boolean" },
    { id: "txn-json-array", label: "JSON Array" },
    { id: "txn-string-numeric", label: "String Numeric" },
    { id: "txn-ttl-per-item", label: "Per-item TTL" },
    { id: "txn-no-partial-mutation", label: "No Partial Mutation" },
    { id: "txn-empty-sdk-rejected", label: "Empty Transaction" },
  ],
});

void run();

async function run(): Promise<void> {
  try {
    setStatus("running", "running");
    const config = readConfig();
    const bucketId = requireBucketId(config);
    const auth = requireAuth(config);
    const firstKey = requireKey(config, "firstKey");
    const secondKey = requireKey(config, "secondKey");
    const deletedKey = requireKey(config, "deletedKey");
    const counterKey = requireKey(config, "counterKey");
    const jsonIntegerKey = requireKey(config, "jsonIntegerKey");
    const jsonBooleanKey = requireKey(config, "jsonBooleanKey");
    const jsonArrayKey = requireKey(config, "jsonArrayKey");
    const stringNumericKey = requireKey(config, "stringNumericKey");
    const ttlKey = requireKey(config, "ttlKey");
    const partialSetKey = requireKey(config, "partialSetKey");
    const partialDeleteKey = requireKey(config, "partialDeleteKey");
    const client = createKeydock({ baseUrl: config.url, auth });
    const bucket = client.bucket(bucketId);

    setText("bucket-id", bucketId);
    appendLog("posting transaction");
    await bucket.transaction([
      { set: firstKey, value: "one" },
      { set: secondKey, value: { value: "two" } },
      { delete: deletedKey },
    ]);
    setStep("transaction-result", "done", "transaction committed");

    const first = await bucket.getText(firstKey);
    setStep("first-read-result", "done", first);

    const second = await bucket.getJson<{ value: string }>(secondKey);
    setStep("second-read-result", "done", second.value);

    const deleted = await bucket.getTextOrNull(deletedKey);
    setStep("deleted-read-result", "done", String(deleted));

    const counter = await bucket.increment(counterKey, 1);
    setAttr("counter-result", "data-counter-raw", counter.raw);
    setAttr("counter-result", "data-counter-kind", counter.kind);
    if (counter.kind === "integer" && counter.number !== undefined) {
      setAttr("counter-result", "data-counter-number", String(counter.number));
    }
    setStep("counter-result", "done", counter.raw);

    await bucket.transaction([{ set: jsonIntegerKey, value: 42 }]);
    setStep("txn-json-integer", "done", String(await bucket.getJson<number>(jsonIntegerKey)));

    await bucket.transaction([{ set: jsonBooleanKey, value: true }]);
    setStep("txn-json-boolean", "done", String(await bucket.getJson<boolean>(jsonBooleanKey)));

    await bucket.transaction([{ set: jsonArrayKey, value: [1, 2, 3] }]);
    setStep("txn-json-array", "done", JSON.stringify(await bucket.getJson<number[]>(jsonArrayKey)));

    await bucket.transaction([{ set: stringNumericKey, value: "42" }]);
    setStep("txn-string-numeric", "done", await bucket.getText(stringNumericKey));

    await bucket.transaction([{ set: ttlKey, value: "v", ttlSeconds: 1 }]);
    await sleep(2_000);
    setStep("txn-ttl-per-item", "done", String(await bucket.getTextOrNull(ttlKey)));

    await bucket.setText(partialDeleteKey, "delete-denied");
    const writeOnlyToken = await bucket.tokens.create({
      prefix: "partial:",
      permissions: ["write"],
      ttlSeconds: 900,
    });
    const writeOnlyBucket = createKeydock({
      baseUrl: config.url,
      auth: writeOnlyToken.accessToken,
    }).bucket(bucketId);
    const partialError = await captureKeydockError(() =>
      writeOnlyBucket.transaction([
        { set: partialSetKey, value: "must-not-commit" },
        { delete: partialDeleteKey },
      ]),
    );
    setStep(
      "txn-no-partial-mutation",
      "done",
      `${partialError.name}:${partialError.status}:${String(await bucket.getTextOrNull(partialSetKey))}`,
    );

    setStep("txn-empty-sdk-rejected", "done", await captureAnyError(() => bucket.transaction([])));

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function captureKeydockError(operation: () => Promise<unknown>): Promise<KeydockError> {
  try {
    await operation();
  } catch (error) {
    if (error instanceof KeydockError) {
      return error;
    }
    throw error;
  }

  throw new Error("expected operation to fail with KeydockError");
}

async function captureAnyError(operation: () => Promise<unknown>): Promise<string> {
  try {
    await operation();
  } catch (error) {
    if (error instanceof KeydockError) {
      return `${error.name}:${error.status}`;
    }
    if (error instanceof KeydockValidationError) {
      return error.name;
    }
    throw error;
  }

  throw new Error("expected operation to fail");
}
