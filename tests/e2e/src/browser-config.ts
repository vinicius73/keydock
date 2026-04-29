export type BucketCredentials = {
  email: string;
  secretKey: string;
  readKey: string;
  writeKey: string;
  signingKey?: string;
};

export type KeydockE2eConfig = {
  url: string;
  bucketId?: string;
  auth?: string;
  credentials?: BucketCredentials;
  keys?: Record<string, string>;
};

declare global {
  interface Window {
    __KEYDOCK_E2E__?: KeydockE2eConfig;
  }
}

export function readConfig(): KeydockE2eConfig {
  const config = window.__KEYDOCK_E2E__;
  if (config === undefined) {
    throw new Error("window.__KEYDOCK_E2E__ is not configured");
  }
  if (config.url.length === 0) {
    throw new Error("window.__KEYDOCK_E2E__.url must be non-empty");
  }
  return config;
}

export function requireBucketId(config: KeydockE2eConfig): string {
  if (config.bucketId === undefined || config.bucketId.length === 0) {
    throw new Error("window.__KEYDOCK_E2E__.bucketId must be non-empty");
  }
  return config.bucketId;
}

export function requireAuth(config: KeydockE2eConfig): string {
  if (config.auth === undefined || config.auth.length === 0) {
    throw new Error("window.__KEYDOCK_E2E__.auth must be non-empty");
  }
  return config.auth;
}

export function requireCredentials(config: KeydockE2eConfig): BucketCredentials {
  if (config.credentials === undefined) {
    throw new Error("window.__KEYDOCK_E2E__.credentials must be configured");
  }
  return config.credentials;
}

export function requireKey(config: KeydockE2eConfig, name: string): string {
  const key = config.keys?.[name];
  if (key === undefined || key.length === 0) {
    throw new Error(`window.__KEYDOCK_E2E__.keys.${name} must be non-empty`);
  }
  return key;
}
