import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./specs",
  fullyParallel: true,
  timeout: 30_000,
  expect: {
    timeout: 10_000,
  },
  globalSetup: "./support/global-setup.ts",
  globalTeardown: "./support/global-teardown.ts",
  reporter: [["list"], ["html", { open: "never", outputFolder: "playwright-report" }]],
  webServer: {
    command: "bunx vite --host 127.0.0.1 --port 5173",
    url: "http://127.0.0.1:5173/apps/basic/",
    reuseExistingServer: !process.env.CI,
    timeout: 20_000,
  },
  use: {
    baseURL: "http://127.0.0.1:5173",
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
