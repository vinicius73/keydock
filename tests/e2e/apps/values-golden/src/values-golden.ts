import { createKeydock } from "keydock-sdk";

import { readConfig, requireCredentials } from "../../../src/browser-config.js";
import { bucketCreateInput, captureKeydockError } from "../../../src/sdk-test-helpers.js";
import {
  appendLog,
  mountE2eApp,
  renderError,
  setStatus,
  setStep,
  setText,
} from "../../../src/ui.js";

const steps = [
  { id: "create-result", label: "Create" },
  { id: "setText-roundtrip", label: "Text Roundtrip" },
  { id: "setText-empty", label: "Empty Text" },
  { id: "setText-spaces", label: "Text Spaces" },
  { id: "setText-numeric", label: "Numeric Text" },
  { id: "setText-jsonish", label: "JSON-looking Text" },
  { id: "setJson-object", label: "JSON Object" },
  { id: "setJson-null", label: "JSON Null" },
  { id: "getJsonOrNull-null-key", label: "JSON Null OrNull" },
  { id: "setBytes-roundtrip", label: "Bytes Roundtrip" },
  { id: "getTextOrNull-miss", label: "Missing Text OrNull" },
  { id: "getJsonOrNull-miss", label: "Missing JSON OrNull" },
  { id: "getBytesOrNull-miss", label: "Missing Bytes OrNull" },
  { id: "getTextOrNull-forbidden", label: "Forbidden OrNull" },
  { id: "key-slash", label: "Slash Key" },
  { id: "key-percent", label: "Percent Key" },
  { id: "key-space", label: "Space Key" },
  { id: "key-128-bytes", label: "128-byte Key" },
  { id: "key-129-bytes", label: "129-byte Key" },
  { id: "value-16384-bytes", label: "16 KiB Bytes" },
  { id: "value-16385-bytes", label: "16 KiB + 1 Bytes" },
  { id: "delete-existing", label: "Delete Existing" },
  { id: "delete-missing", label: "Delete Missing" },
  { id: "exists-false", label: "Exists False" },
] as const;

mountE2eApp({
  title: "Values Golden",
  description:
    "A browser mini app exercises value roundtrips, missing-key helpers, key encoding, limits, and delete semantics through the installed SDK.",
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
    const readClient = createKeydock({
      baseUrl: config.url,
      auth: credentials.readKey,
    });
    const writeClient = createKeydock({
      baseUrl: config.url,
      auth: credentials.writeKey,
    });

    setStep("create-result", "running", "creating bucket");
    const created = await anonymous.buckets.create(
      bucketCreateInput(credentials, { defaultTtlSeconds: 0 }),
    );
    setText("bucket-id", created.id);
    setStep("create-result", "done", "bucket created");
    appendLog(`created bucket ${created.id}`);

    const adminBucket = admin.bucket(created.id);
    const readBucket = readClient.bucket(created.id);
    const writeBucket = writeClient.bucket(created.id);

    await writeBucket.setText("message", "hello");
    setStep("setText-roundtrip", "done", await readBucket.getText("message"));

    await writeBucket.setText("empty", "");
    setStep("setText-empty", "done", await readBucket.getText("empty"));

    await writeBucket.setText("spaces", "  hi  ");
    setStep("setText-spaces", "done", await readBucket.getText("spaces"));

    await writeBucket.setText("numeric", "42");
    setStep("setText-numeric", "done", await readBucket.getText("numeric"));

    await writeBucket.setText("jsonish", '{"a":1}');
    setStep("setText-jsonish", "done", await readBucket.getText("jsonish"));

    await writeBucket.setJson("object", { name: "Ana", ok: true });
    const objectValue = await readBucket.getJson<{ name: string; ok: boolean }>("object");
    setStep("setJson-object", "done", `${objectValue.name}:${String(objectValue.ok)}`);

    await writeBucket.setJson("null-key", null);
    const nullValue = await readBucket.getJson("null-key");
    setStep("setJson-null", "done", String(nullValue));

    const nullableJson = await readBucket.getJsonOrNull("null-key");
    setStep("getJsonOrNull-null-key", "done", String(nullableJson));

    await writeBucket.setBytes("bytes", new Uint8Array([0xff, 0x00, 0xfe]));
    const bytes = await readBucket.getBytes("bytes");
    setStep("setBytes-roundtrip", "done", bytes.join(",") === "255,0,254" ? "equal" : "different");

    setStep("getTextOrNull-miss", "done", String(await readBucket.getTextOrNull("__miss__")));
    setStep("getJsonOrNull-miss", "done", String(await readBucket.getJsonOrNull("__miss__")));
    setStep("getBytesOrNull-miss", "done", String(await readBucket.getBytesOrNull("__miss__")));

    await adminBucket.setText("forbidden:key", "secret");
    const writeOnlyToken = await adminBucket.tokens.create({
      prefix: "forbidden:",
      permissions: ["write"],
      ttlSeconds: 900,
    });
    const writeOnlyBucket = createKeydock({
      baseUrl: config.url,
      auth: writeOnlyToken.accessToken,
    }).bucket(created.id);
    const forbidden = await captureKeydockError(() =>
      writeOnlyBucket.getTextOrNull("forbidden:key"),
    );
    setStep("getTextOrNull-forbidden", "done", `${forbidden.name}:${forbidden.status}`);

    await writeBucket.setText("user/42", "v");
    setStep("key-slash", "done", await readBucket.getText("user/42"));

    await writeBucket.setText("a%b", "v");
    setStep("key-percent", "done", await readBucket.getText("a%b"));

    await writeBucket.setText("hello world", "v");
    setStep("key-space", "done", await readBucket.getText("hello world"));

    const maxKey = "a".repeat(128);
    await writeBucket.setText(maxKey, "stored");
    setStep("key-128-bytes", "done", await readBucket.getText(maxKey));

    const tooLongKey = "a".repeat(129);
    const keyError = await captureKeydockError(() => writeBucket.setText(tooLongKey, "nope"));
    setStep("key-129-bytes", "done", `${keyError.name}:${keyError.status}`);

    await writeBucket.setBytes("big", new Uint8Array(16_384));
    setStep("value-16384-bytes", "done", "stored");

    const valueError = await captureKeydockError(() =>
      writeBucket.setBytes("too-big", new Uint8Array(16_385)),
    );
    setStep("value-16385-bytes", "done", `${valueError.name}:${valueError.status}`);

    await adminBucket.delete("message");
    setStep("delete-existing", "done", String(await readBucket.exists("message")));

    const deleteMissing = await captureKeydockError(() => adminBucket.delete("__miss__"));
    setStep("delete-missing", "done", `${deleteMissing.name}:${deleteMissing.status}`);

    setStep("exists-false", "done", String(await readBucket.exists("__never__")));

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}
