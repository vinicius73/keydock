import { expect, test } from "@playwright/test";

import type { KeydockE2eConfig } from "../src/browser-config.js";
import { e2eBaseUrl } from "../support/sdk-admin.js";
import { uniqueBucketData } from "../support/test-data.js";

test.describe("anonymous auth matrix golden SDK browser coverage", () => {
  test("covers anonymous access for documented key combinations", async ({ page }) => {
    const config: KeydockE2eConfig = {
      url: e2eBaseUrl(),
      credentials: uniqueBucketData("auth-anon-matrix"),
    };

    await page.addInitScript((input) => {
      window.__KEYDOCK_E2E__ = input;
    }, config);

    await page.goto("/apps/auth-anonymous-matrix/");

    await expect(page.getByTestId("app-status")).toHaveText("done");

    await expect(page.getByTestId("no-keys-read")).toHaveText("ok");
    await expect(page.getByTestId("no-keys-list")).toHaveText("ok");
    await expect(page.getByTestId("no-keys-write")).toHaveText("ok");
    await expect(page.getByTestId("no-keys-delete")).toHaveText("ok");

    await expect(page.getByTestId("read-only-read")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("read-only-list")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("read-only-write")).toHaveText("ok");
    await expect(page.getByTestId("read-only-delete")).toHaveText("KeydockError:401");

    await expect(page.getByTestId("write-only-read")).toHaveText("KeydockError:404");
    await expect(page.getByTestId("write-only-list")).toHaveText("ok");
    await expect(page.getByTestId("write-only-write")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("write-only-delete")).toHaveText("KeydockError:401");

    await expect(page.getByTestId("secret-read-read")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("secret-read-list")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("secret-read-write")).toHaveText("ok");
    await expect(page.getByTestId("secret-read-delete")).toHaveText("KeydockError:401");

    await expect(page.getByTestId("secret-write-read")).toHaveText("ok");
    await expect(page.getByTestId("secret-write-list")).toHaveText("ok");
    await expect(page.getByTestId("secret-write-write")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("secret-write-delete")).toHaveText("KeydockError:401");

    await expect(page.getByTestId("read-write-read")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("read-write-list")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("read-write-write")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("read-write-delete")).toHaveText("KeydockError:401");

    await expect(page.getByTestId("signing-only-read")).toHaveText("ok");
    await expect(page.getByTestId("signing-only-list")).toHaveText("ok");
    await expect(page.getByTestId("signing-only-write")).toHaveText("ok");
    await expect(page.getByTestId("signing-only-delete")).toHaveText("ok");
  });
});
