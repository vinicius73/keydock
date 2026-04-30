import { expect, test } from "@playwright/test";

import type { KeydockE2eConfig } from "../src/browser-config.js";
import {
  deleteBucketBestEffort,
  e2eBaseUrl,
  type CreatedBucketFixture,
} from "../support/sdk-admin.js";
import { uniqueBucketData } from "../support/test-data.js";

test.describe("values golden SDK browser coverage", () => {
  let cleanupTarget: CreatedBucketFixture | undefined;

  test.afterEach(async ({ page }) => {
    if (cleanupTarget !== undefined) {
      const bucketId = await page
        .getByTestId("bucket-id")
        .textContent()
        .catch(() => undefined);
      if (
        bucketId !== undefined &&
        bucketId !== null &&
        bucketId !== "not-created"
      ) {
        cleanupTarget = {
          id: bucketId,
          credentials: cleanupTarget.credentials,
        };
      }
    }
    await deleteBucketBestEffort(cleanupTarget);
    cleanupTarget = undefined;
  });

  test("covers value roundtrips, key limits, OrNull, and delete semantics", async ({
    page,
  }) => {
    const credentials = uniqueBucketData("values");
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

    await page.goto("/apps/values-golden/");

    await expect(page.getByTestId("app-status")).toHaveText("done");
    await expect(page.getByTestId("create-result")).toHaveText(
      "bucket created",
    );
    await expect(page.getByTestId("setText-roundtrip")).toHaveText("hello");
    await expect(page.getByTestId("setText-empty")).toHaveText("");
    await expect(page.getByTestId("setText-spaces")).toHaveText("  hi  ");
    await expect(page.getByTestId("setText-numeric")).toHaveText("42");
    await expect(page.getByTestId("setText-jsonish")).toHaveText('{"a":1}');
    await expect(page.getByTestId("setJson-object")).toHaveText("Ana:true");
    await expect(page.getByTestId("setJson-null")).toHaveText("null");
    await expect(page.getByTestId("getJsonOrNull-null-key")).toHaveText("null");
    await expect(page.getByTestId("setBytes-roundtrip")).toHaveText("equal");
    await expect(page.getByTestId("getTextOrNull-miss")).toHaveText("null");
    await expect(page.getByTestId("getJsonOrNull-miss")).toHaveText(
      "undefined",
    );
    await expect(page.getByTestId("getBytesOrNull-miss")).toHaveText("null");
    await expect(page.getByTestId("getTextOrNull-forbidden")).toHaveText(
      "KeydockError:403",
    );
    await expect(page.getByTestId("key-slash")).toHaveText("v");
    await expect(page.getByTestId("key-percent")).toHaveText("v");
    await expect(page.getByTestId("key-space")).toHaveText("v");
    await expect(page.getByTestId("key-128-bytes")).toHaveText("stored");
    await expect(page.getByTestId("key-129-bytes")).toHaveText(
      "KeydockError:400",
    );
    await expect(page.getByTestId("value-16384-bytes")).toHaveText("stored");
    await expect(page.getByTestId("value-16385-bytes")).toHaveText(
      "KeydockError:400",
    );
    await expect(page.getByTestId("delete-existing")).toHaveText("false");
    await expect(page.getByTestId("delete-missing")).toHaveText(
      "KeydockError:404",
    );
    await expect(page.getByTestId("exists-false")).toHaveText("false");
  });
});
