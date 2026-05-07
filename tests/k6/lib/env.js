export function requiredEnv(name) {
  const value = __ENV[name];
  if (value === undefined || value === null || value === "") {
    throw new Error(`missing required env var: ${name}`);
  }
  return value;
}

export function optionalEnv(name, defaultValue) {
  const value = __ENV[name];
  if (value === undefined || value === null || value === "")
    return defaultValue;
  return value;
}

export const BASE_URL = requiredEnv("KEYDOCK_BASE_URL").replace(/\/+$/, "");
export const RUN_ID = optionalEnv(
  "RUN_ID",
  `k6-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`,
);

export function cleanupEnabled() {
  const raw = optionalEnv("K6_CLEANUP", "false");
  return raw !== "0" && raw.toLowerCase() !== "false";
}
