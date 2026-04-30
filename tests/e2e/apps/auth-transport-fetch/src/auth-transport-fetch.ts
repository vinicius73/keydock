import { createKeydock } from "keydock-sdk";

import { readConfig, requireCredentials } from "../../../src/browser-config.js";
import { bucketCreateInput } from "../../../src/sdk-test-helpers.js";
import { mountE2eApp, renderError, setStatus, setStep, setText } from "../../../src/ui.js";

const API_PREFIX = "/api/v1";

const steps = [
  { id: "transport-access-token", label: "Query access_token" },
  { id: "transport-key", label: "Query key" },
  { id: "transport-query-priority", label: "Query Priority" },
  { id: "transport-bearer-wins-query", label: "Bearer Wins Query" },
  { id: "transport-basic-username", label: "Basic Username" },
] as const;

mountE2eApp({
  title: "Auth Transport Fetch",
  description:
    "A browser mini app verifies non-SDK credential transports using raw fetch requests.",
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
    const created = await anonymous.buckets.create(
      bucketCreateInput(credentials, { defaultTtlSeconds: 0 }),
    );
    setText("bucket-id", created.id);

    const admin = createKeydock({
      baseUrl: config.url,
      auth: credentials.secretKey,
    });
    const adminBucket = admin.bucket(created.id);

    try {
      await adminBucket.setText("transport-target", "ok");
      const target = keyUrl(config.url, created.id, "transport-target");

      setStep(
        "transport-access-token",
        "done",
        await readViaFetch(withQuery(target, { access_token: credentials.readKey })),
      );
      setStep(
        "transport-key",
        "done",
        await readViaFetch(withQuery(target, { key: credentials.readKey })),
      );
      setStep(
        "transport-query-priority",
        "done",
        await readViaFetch(
          withQuery(target, {
            key: "wrong-credential",
            access_token: credentials.readKey,
          }),
        ),
      );
      setStep(
        "transport-bearer-wins-query",
        "done",
        await readViaFetch(withQuery(target, { access_token: "wrong-credential" }), {
          Authorization: `Bearer ${credentials.readKey}`,
        }),
      );
      setStep(
        "transport-basic-username",
        "done",
        await readViaFetch(target, {
          Authorization: `Basic ${btoa(`${credentials.readKey}:ignored-password`)}`,
        }),
      );
    } finally {
      await admin.buckets.delete(created.id);
    }

    setStatus("done", "done");
  } catch (error) {
    renderError(error);
  }
}

async function readViaFetch(url: string, headers: Record<string, string> = {}): Promise<string> {
  const response = await fetch(url, { headers });
  if (!response.ok) {
    return String(response.status);
  }

  return `${response.status}:${await response.text()}`;
}

function withQuery(url: string, params: Record<string, string>): string {
  const target = new URL(url);
  for (const [key, value] of Object.entries(params)) {
    target.searchParams.set(key, value);
  }
  return target.toString();
}

function keyUrl(baseUrl: string, bucketId: string, key: string): string {
  return `${normalizeBaseUrl(baseUrl)}${encodeURIComponent(bucketId)}/${encodeURIComponent(key)}`;
}

function normalizeBaseUrl(baseUrl: string): string {
  const url = new URL(baseUrl);
  const pathname = url.pathname.replace(/\/+$/, "");

  if (pathname === "" || pathname === "/") {
    url.pathname = `${API_PREFIX}/`;
  } else if (pathname === API_PREFIX) {
    url.pathname = `${API_PREFIX}/`;
  } else {
    url.pathname = `${pathname}${API_PREFIX}/`;
  }

  url.search = "";
  url.hash = "";

  return url.toString();
}
