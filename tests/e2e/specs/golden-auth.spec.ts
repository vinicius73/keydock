import { expect, test } from "@playwright/test";

import type { KeydockE2eConfig } from "../src/browser-config.js";
import {
  deleteBucketBestEffort,
  e2eBaseUrl,
  type CreatedBucketFixture,
} from "../support/sdk-admin.js";
import { uniqueBucketData } from "../support/test-data.js";

test.describe("auth matrix golden SDK browser coverage", () => {
  let cleanupTarget: CreatedBucketFixture | undefined;

  test.afterEach(async ({ page }) => {
    if (cleanupTarget !== undefined) {
      const bucketId = await page
        .getByTestId("bucket-id")
        .textContent()
        .catch(() => undefined);
      if (bucketId !== undefined && bucketId !== null && bucketId !== "not-created") {
        cleanupTarget = {
          id: bucketId,
          credentials: cleanupTarget.credentials,
        };
      }
    }
    await deleteBucketBestEffort(cleanupTarget);
    cleanupTarget = undefined;
  });

  test("covers admin, read, write, anonymous, wrong, and public access", async ({ page }) => {
    const credentials = uniqueBucketData("auth");
    cleanupTarget = {
      id: "not-created",
      credentials,
    };
    const config: KeydockE2eConfig = {
      url: e2eBaseUrl(),
      credentials,
    };

    await page.addInitScript((input) => {
      window.__KEYDOCK_E2E__ = input;
    }, config);

    await page.goto("/apps/auth-matrix/");

    await expect(page.getByTestId("app-status")).toHaveText("done");
    await expect(page.getByTestId("create-result")).toHaveText("bucket created");
    await expect(page.getByTestId("admin-read")).toHaveText("ok");
    await expect(page.getByTestId("admin-write")).toHaveText("ok");
    await expect(page.getByTestId("admin-list")).toHaveText("ok");
    await expect(page.getByTestId("admin-delete")).toHaveText("ok");
    await expect(page.getByTestId("readKey-read")).toHaveText("ok");
    await expect(page.getByTestId("readKey-write")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("readKey-list")).toHaveText("ok");
    await expect(page.getByTestId("readKey-delete")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("writeKey-read")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("writeKey-write")).toHaveText("ok");
    await expect(page.getByTestId("writeKey-list")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("anon-read")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("anon-write")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("wrong-cred")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("missing-bucket")).toHaveText("KeydockError:404");
    await expect(page.getByTestId("public-anon-read")).toHaveText("ok");
    await expect(page.getByTestId("public-anon-write")).toHaveText("ok");
    await expect(page.getByTestId("public-anon-list")).toHaveText("ok");
    await expect(page.getByTestId("public-anon-delete")).toHaveText("KeydockError:401");
  });
});
