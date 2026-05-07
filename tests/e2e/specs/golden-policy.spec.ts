import { expect, test } from "@playwright/test";

import type { KeydockE2eConfig } from "../src/browser-config.js";
import {
  deleteBucketBestEffort,
  e2eBaseUrl,
  type CreatedBucketFixture,
} from "../support/sdk-admin.js";
import { uniqueBucketData } from "../support/test-data.js";

test.describe("policy golden SDK browser coverage", () => {
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

  test("covers bucket policy lifecycle and admin-only operations", async ({ page }) => {
    const credentials = uniqueBucketData("policy");
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

    await page.goto("/apps/policy-golden/");

    await expect(page.getByTestId("app-status")).toHaveText("done");
    await expect(page.getByTestId("create-policy-shape")).toHaveText(
      "secret:true,read:true,write:true,sign:true,gen:0,ttl:3600",
    );
    await expect(page.getByTestId("create-no-default-ttl")).toHaveText("604800");
    await expect(page.getByTestId("create-default-ttl-zero")).toHaveText("0");
    await expect(page.getByTestId("policy-secrets-absent")).toHaveText("absent");
    await expect(page.getByTestId("update-rotate-read-key")).toHaveText("ok");
    await expect(page.getByTestId("update-clear-write-key")).toHaveText("false");
    await expect(page.getByTestId("update-clear-read-key")).toHaveText("ok");
    await expect(page.getByTestId("update-clear-signing")).toHaveText("sign:false,gen:1");
    await expect(page.getByTestId("update-rotate-signing")).toHaveText("0");
    await expect(page.getByTestId("update-secret-key-null")).toHaveText("KeydockValidationError");
    await expect(page.getByTestId("update-default-ttl")).toHaveText("120");
    await expect(page.getByTestId("update-clear-default-ttl")).toHaveText("undefined");
    await expect(page.getByTestId("bucket-exists-true")).toHaveText("true");
    await expect(page.getByTestId("bucket-exists-false")).toHaveText("false");
    await expect(page.getByTestId("bucket-delete")).toHaveText("false");
    await expect(page.getByTestId("policy-non-admin-forbidden")).toHaveText("KeydockError:403");
    await expect(page.getByTestId("policy-non-admin-head")).toHaveText("KeydockError:403");
  });
});
