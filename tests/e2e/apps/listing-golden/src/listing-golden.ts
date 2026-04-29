import { createKeydock, KeydockError } from "keydock-sdk";

import { readConfig, requireCredentials } from "../../../src/browser-config.js";
import { mountE2eApp, renderError, setStatus, setStep, setText } from "../../../src/ui.js";

const steps = [
  { id: "create-result", label: "Create" },
  { id: "list-empty", label: "Empty List" },
  { id: "list-lexicographic", label: "Lexicographic" },
  { id: "list-reverse", label: "Reverse" },
  { id: "list-prefix", label: "Prefix" },
  { id: "list-limit", label: "Limit" },
  { id: "list-skip", label: "Skip" },
  { id: "listEntries-text", label: "Entry Text" },
  { id: "listEntries-json", label: "Entry JSON" },
  { id: "list-no-enumerate", label: "No Enumerate" },
  { id: "list-anon-restricted", label: "Anonymous Restricted" },
  { id: "list-anon-public", label: "Anonymous Public" },
  { id: "list-scoped-compatible", label: "Scoped Compatible" },
  { id: "list-scoped-prefix-override", label: "Scoped Narrowed" },
  { id: "list-scoped-incompatible", label: "Scoped Incompatible" },
  { id: "list-expired-not-shown", label: "Expired Excluded" },
] as const;

mountE2eApp({
  title: "Listing Golden",
  description:
    "A browser mini app verifies SDK listing options, entry decoding, auth requirements, scoped prefixes, and expired-key filtering.",
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
    const admin = createKeydock({ baseUrl: config.url, auth: credentials.secretKey });
    const readClient = createKeydock({ baseUrl: config.url, auth: credentials.readKey });
    const writeClient = createKeydock({ baseUrl: config.url, auth: credentials.writeKey });

    setStep("create-result", "running", "creating bucket");
    const createInput = {
      email: credentials.email,
      secretKey: credentials.secretKey,
      readKey: credentials.readKey,
      writeKey: credentials.writeKey,
      defaultTtlSeconds: 0,
    };
    const created = await anonymous.buckets.create(
      credentials.signingKey === undefined
        ? createInput
        : {
            ...createInput,
            signingKey: credentials.signingKey,
          },
    );
    setText("bucket-id", created.id);
    setStep("create-result", "done", "bucket created");

    const adminBucket = admin.bucket(created.id);
    const readBucket = readClient.bucket(created.id);
    const writeBucket = writeClient.bucket(created.id);
    const anonymousBucket = anonymous.bucket(created.id);

    setStep("list-empty", "done", JSON.stringify(await readBucket.listKeys()));

    await adminBucket.setText("c", "c");
    await adminBucket.setText("a", "a");
    await adminBucket.setText("b", "b");
    setStep("list-lexicographic", "done", (await readBucket.listKeys()).join(","));
    setStep("list-reverse", "done", (await readBucket.listKeys({ reverse: true })).join(","));

    await adminBucket.setText("foo:1", "1");
    await adminBucket.setText("foo:2", "2");
    await adminBucket.setText("bar:1", "1");
    setStep("list-prefix", "done", (await readBucket.listKeys({ prefix: "foo:" })).join(","));

    await seedText(adminBucket, ["k0", "k1", "k2", "k3"]);
    setStep(
      "list-limit",
      "done",
      String((await readBucket.listKeys({ prefix: "k", limit: 2 })).length),
    );
    setStep(
      "list-skip",
      "done",
      (await readBucket.listKeys({ prefix: "k", skip: 1, limit: 2 })).join(","),
    );

    await adminBucket.setText("msg", "hello");
    await adminBucket.setJson("obj", { x: 1 });
    const textEntry = (await readBucket.listEntries({ prefix: "msg" }))[0];
    const jsonEntry = (await readBucket.listEntries({ prefix: "obj" }))[0];
    setStep("listEntries-text", "done", `${textEntry?.key}=${String(textEntry?.value)}`);
    setStep("listEntries-json", "done", `${jsonEntry?.key}=${jsonValue(jsonEntry?.value)}`);

    const writeList = await captureKeydockError(() => writeBucket.listKeys());
    setStep("list-no-enumerate", "done", `${writeList.name}:${writeList.status}`);

    const anonRestricted = await captureKeydockError(() => anonymousBucket.listKeys());
    setStep("list-anon-restricted", "done", `${anonRestricted.name}:${anonRestricted.status}`);

    const publicBucketId = await createPublicBucket(config.url, credentials);
    try {
      const publicAdmin = createKeydock({
        baseUrl: config.url,
        auth: `${credentials.secretKey}-public`,
      }).bucket(publicBucketId);
      await publicAdmin.setText("public:key", "visible");
      const publicKeys = await anonymous.bucket(publicBucketId).listKeys();
      setStep("list-anon-public", "done", publicKeys.join(","));
    } finally {
      await createKeydock({
        baseUrl: config.url,
        auth: `${credentials.secretKey}-public`,
      }).buckets.delete(publicBucketId);
    }

    await adminBucket.setText("scope:a", "a");
    await adminBucket.setText("scope:b", "b");
    await adminBucket.setText("other:a", "a");
    const scopedToken = await adminBucket.tokens.create({
      prefix: "scope:",
      permissions: ["enumerate"],
      ttlSeconds: 900,
    });
    const scopedBucket = createKeydock({
      baseUrl: config.url,
      auth: scopedToken.accessToken,
    }).bucket(created.id);
    setStep(
      "list-scoped-compatible",
      "done",
      (await scopedBucket.listKeys({ prefix: "scope:" })).join(","),
    );

    await adminBucket.setText("a:b1", "1");
    await adminBucket.setText("a:c1", "1");
    await adminBucket.setText("b:a1", "1");
    const prefixToken = await adminBucket.tokens.create({
      prefix: "a:",
      permissions: ["enumerate"],
      ttlSeconds: 900,
    });
    const prefixBucket = createKeydock({
      baseUrl: config.url,
      auth: prefixToken.accessToken,
    }).bucket(created.id);
    setStep(
      "list-scoped-prefix-override",
      "done",
      (await prefixBucket.listKeys({ prefix: "a:b" })).join(","),
    );
    setStep(
      "list-scoped-incompatible",
      "done",
      JSON.stringify(await prefixBucket.listKeys({ prefix: "b:" })),
    );

    await adminBucket.setText("ttl:key", "gone", { ttlSeconds: 1 });
    await sleep(2_000);
    setStep(
      "list-expired-not-shown",
      "done",
      JSON.stringify(await readBucket.listKeys({ prefix: "ttl:" })),
    );

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}

async function seedText(
  bucket: { setText: (key: string, value: string) => Promise<void> },
  keys: string[],
) {
  for (const key of keys) {
    await bucket.setText(key, key);
  }
}

async function createPublicBucket(
  baseUrl: string,
  credentials: { email: string; secretKey: string },
): Promise<string> {
  const client = createKeydock({ baseUrl });
  const created = await client.buckets.create({
    email: `public-${credentials.email}`,
    secretKey: `${credentials.secretKey}-public`,
    defaultTtlSeconds: 0,
  });
  return created.id;
}

function jsonValue(value: unknown): string {
  if (typeof value === "object" && value !== null && "x" in value) {
    return String(value.x);
  }
  return String(value);
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
