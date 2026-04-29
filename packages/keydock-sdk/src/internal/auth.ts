import type { KeydockAuth } from "../types.js";

export async function resolveAuth(auth: KeydockAuth | undefined): Promise<string | undefined> {
  if (auth === undefined) {
    return undefined;
  }

  if (typeof auth === "string") {
    return auth;
  }

  return auth();
}
