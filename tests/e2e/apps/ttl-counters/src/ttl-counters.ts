import { createKeydock } from "keydock-sdk";
import type { CounterValue } from "keydock-sdk";

import { readConfig, requireCredentials } from "../../../src/browser-config.js";
import {
  captureAnyError,
  captureKeydockError,
  sleep,
  withTemporaryBucket,
} from "../../../src/sdk-test-helpers.js";
import { mountE2eApp, renderError, setStatus, setStep, setText } from "../../../src/ui.js";

const steps = [
  { id: "create-result", label: "Create" },
  { id: "setText-ttl-expires", label: "Text TTL" },
  { id: "setJson-ttl-expires", label: "JSON TTL" },
  { id: "setBytes-ttl-expires", label: "Bytes TTL" },
  { id: "ttl-zero-no-expiry", label: "TTL Zero" },
  { id: "ttl-renewal", label: "TTL Renewal" },
  { id: "default-ttl-604800", label: "Default TTL" },
  { id: "default-ttl-zero", label: "Default TTL Zero" },
  { id: "ttl-expired-excluded-from-list", label: "Expired List" },
  { id: "counter-from-zero-int", label: "Counter From Zero" },
  { id: "counter-negative", label: "Counter Negative" },
  { id: "counter-add-int", label: "Counter Add Int" },
  { id: "counter-int-plus-float", label: "Counter Float" },
  { id: "counter-bigint-safe", label: "Counter BigInt Safe" },
  { id: "counter-bigint-unsafe", label: "Counter BigInt Unsafe" },
  { id: "counter-zero-rejected", label: "Counter Zero" },
  { id: "counter-nan-rejected", label: "Counter NaN" },
  { id: "counter-non-numeric", label: "Counter Non-numeric" },
  { id: "counter-with-ttl", label: "Counter TTL" },
] as const;

mountE2eApp({
  title: "TTL and Counters",
  description:
    "A browser mini app verifies write TTLs, default TTL policy values, and counter parsing/validation through the SDK.",
  bucketId: "not-created",
  steps: [...steps],
});

void run();

async function run(): Promise<void> {
  try {
    setStatus("running", "running");
    const config = readConfig();
    const credentials = requireCredentials(config);
    const anonymous = createKeydock({ baseUrl: config.url });
    const admin = createKeydock({
      baseUrl: config.url,
      auth: credentials.secretKey,
    });

    const created = await anonymous.buckets.create({
      email: credentials.email,
      secretKey: credentials.secretKey,
      readKey: credentials.readKey,
      writeKey: credentials.writeKey,
      defaultTtlSeconds: 0,
    });
    setText("bucket-id", created.id);
    setStep("create-result", "done", "bucket created");

    const bucket = admin.bucket(created.id);

    await bucket.setText("ttl:text", "v", { ttlSeconds: 1 });
    await bucket.setJson("ttl:json", { x: 1 }, { ttlSeconds: 1 });
    await bucket.setBytes("ttl:bytes", new Uint8Array([1, 2, 3]), {
      ttlSeconds: 1,
    });
    await bucket.setText("ttl:zero", "v", { ttlSeconds: 0 });
    await bucket.setText("ttl:list", "gone", { ttlSeconds: 1 });
    await bucket.increment("counter:ttl", 1, { ttlSeconds: 1 });
    await sleep(2_000);

    setStep("setText-ttl-expires", "done", String(await bucket.getTextOrNull("ttl:text")));
    setStep("setJson-ttl-expires", "done", String(await bucket.getJsonOrNull("ttl:json")));
    setStep("setBytes-ttl-expires", "done", String(await bucket.getBytesOrNull("ttl:bytes")));
    setStep("ttl-zero-no-expiry", "done", await bucket.getText("ttl:zero"));
    setStep(
      "ttl-expired-excluded-from-list",
      "done",
      JSON.stringify(await bucket.listKeys({ prefix: "ttl:list" })),
    );
    setStep("counter-with-ttl", "done", String(await bucket.getTextOrNull("counter:ttl")));

    await bucket.setText("ttl:renewal", "v", { ttlSeconds: 3 });
    await sleep(1_000);
    await bucket.setText("ttl:renewal", "v", { ttlSeconds: 3 });
    await sleep(2_000);
    setStep("ttl-renewal", "done", await bucket.getText("ttl:renewal"));

    await withTemporaryBucket(
      config.url,
      {
        email: `default-${credentials.email}`,
        secretKey: `${credentials.secretKey}-default`,
      },
      async (client, bucketId) => {
        setStep(
          "default-ttl-604800",
          "done",
          String((await client.buckets.getPolicy(bucketId)).defaultTtlSeconds),
        );
      },
    );

    await withTemporaryBucket(
      config.url,
      {
        email: `default-zero-${credentials.email}`,
        secretKey: `${credentials.secretKey}-default-zero`,
        defaultTtlSeconds: 0,
      },
      async (client, bucketId) => {
        const tempBucket = client.bucket(bucketId);
        await tempBucket.setText("default-zero", "v");
        await sleep(2_000);
        setStep("default-ttl-zero", "done", await tempBucket.getText("default-zero"));
      },
    );

    setStep(
      "counter-from-zero-int",
      "done",
      counterSummary(await bucket.increment("counter:int", 1)),
    );
    setStep(
      "counter-negative",
      "done",
      counterSummary(await bucket.increment("counter:negative", -3)),
    );

    await bucket.setText("counter:add", "10");
    setStep("counter-add-int", "done", counterSummary(await bucket.increment("counter:add", 5)));

    await bucket.setText("counter:float", "10");
    setStep(
      "counter-int-plus-float",
      "done",
      counterSummary(await bucket.increment("counter:float", 0.5)),
    );

    setStep(
      "counter-bigint-safe",
      "done",
      counterSummary(await bucket.increment("counter:bigint-safe", 42n)),
    );
    setStep(
      "counter-bigint-unsafe",
      "done",
      counterSummary(await bucket.increment("counter:bigint-unsafe", 9_007_199_254_740_993n)),
    );

    setStep("counter-zero-rejected", "done", await captureAnyError(() => bucket.increment("c", 0)));
    setStep(
      "counter-nan-rejected",
      "done",
      await captureAnyError(() => bucket.increment("c", Number.NaN)),
    );

    await bucket.setText("counter:non-numeric", "hello");
    const nonNumeric = await captureKeydockError(() => bucket.increment("counter:non-numeric", 1));
    setStep("counter-non-numeric", "done", `${nonNumeric.name}:${nonNumeric.status}`);

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}

function counterSummary(counter: CounterValue): string {
  const numberPart =
    "number" in counter && counter.number !== undefined ? `,number:${counter.number}` : "";
  return `raw:${counter.raw},kind:${counter.kind}${numberPart}`;
}
