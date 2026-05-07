import { expect, test } from "@playwright/test";

import type { KeydockE2eConfig } from "../src/browser-config.js";
import { e2eBaseUrl } from "../support/sdk-admin.js";
import { uniqueBucketData } from "../support/test-data.js";

const expectedAnonymousMatrix = {
  "secret-only-read": "ok",
  "secret-only-list": "ok",
  "secret-only-write": "ok",
  "secret-only-delete": "KeydockError:401",

  "secret-read-read": "KeydockError:401",
  "secret-read-list": "KeydockError:401",
  "secret-read-write": "ok",
  "secret-read-delete": "KeydockError:401",

  "secret-write-read": "ok",
  "secret-write-list": "ok",
  "secret-write-write": "KeydockError:401",
  "secret-write-delete": "KeydockError:401",

  "all-three-read": "KeydockError:401",
  "all-three-list": "KeydockError:401",
  "all-three-write": "KeydockError:401",
  "all-three-delete": "KeydockError:401",

  "secret-signing-read": "ok",
  "secret-signing-list": "ok",
  "secret-signing-write": "ok",
  "secret-signing-delete": "KeydockError:401",
} as const;

test.describe("anonymous auth matrix golden SDK browser coverage", () => {
  test("covers cleanup-safe anonymous access for documented key combinations", async ({ page }) => {
    const config: KeydockE2eConfig = {
      url: e2eBaseUrl(),
      credentials: uniqueBucketData("auth-anon-matrix"),
    };

    await page.addInitScript((input) => {
      window.__KEYDOCK_E2E__ = input;
    }, config);

    await page.goto("/apps/auth-anonymous-matrix/");

    await expect(page.getByTestId("app-status")).toHaveText("done");

    for (const [stepId, expected] of Object.entries(expectedAnonymousMatrix)) {
      await expect(page.getByTestId(stepId)).toHaveText(expected);
    }
  });
});
