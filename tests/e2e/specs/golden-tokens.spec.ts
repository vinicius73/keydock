import { expect, test } from "@playwright/test";

import type { KeydockE2eConfig } from "../src/browser-config.js";
import {
  deleteBucketBestEffort,
  e2eBaseUrl,
  type CreatedBucketFixture,
} from "../support/sdk-admin.js";
import { uniqueBucketData } from "../support/test-data.js";

test.describe("tokens golden SDK browser coverage", () => {
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

  test("covers scoped token permissions, expiry, rotation, and validation", async ({ page }) => {
    const credentials = uniqueBucketData("tokens");
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

    await page.goto("/apps/tokens-golden/");

    await expect(page.getByTestId("app-status")).toHaveText("done");
    await expect(page.getByTestId("create-result")).toHaveText("bucket created");
    await expect(page.getByTestId("token-read-within")).toHaveText("ok");
    await expect(page.getByTestId("token-read-outside")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("token-read-no-write")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("token-write-within")).toHaveText("ok");
    await expect(page.getByTestId("token-write-no-read")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("token-write-no-delete")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("token-delete-within")).toHaveText("ok");
    await expect(page.getByTestId("token-delete-outside")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("token-enumerate-within")).toHaveText("ok");
    await expect(page.getByTestId("token-enumerate-no-read")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("token-expired")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("token-wrong-bucket")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("token-post-rotation")).toHaveText("KeydockError:401");
    await expect(page.getByTestId("token-new-after-rotation")).toHaveText("ok");
    await expect(page.getByTestId("token-no-signing-key")).toHaveText("KeydockError:503");
    await expect(page.getByTestId("sdk-empty-prefix")).toHaveText("KeydockValidationError");
    await expect(page.getByTestId("sdk-zero-ttl")).toHaveText("KeydockValidationError");
    await expect(page.getByTestId("sdk-empty-permissions")).toHaveText("KeydockValidationError");
  });
});
