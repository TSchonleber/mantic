import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 30_000,
  webServer: {
    command: "pnpm --filter mantic-desktop dev",
    url: "http://localhost:1420",
    timeout: 30_000,
    reuseExistingServer: !process.env.CI,
    cwd: "../..",
  },
  use: {
    baseURL: "http://localhost:1420",
    trace: "off",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
