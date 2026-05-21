import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const fakeInvoke = async (cmd: string) => {
      switch (cmd) {
        case "current_account":
          return {
            account_id: "acct_smoke",
            tier: "pro",
            expires_at: Math.floor(Date.now() / 1000) + 3600,
          };
        case "wallet_status":
          return {
            session_pubkey_b58: "s",
            master_pubkey_b58: "m",
            authorization: {
              master_pubkey_b58: "m",
              session_pubkey_b58: "s",
              message: "msg",
              signature_b58: "sig",
              signed_at: "now",
            },
          };
        case "agent_list":
          return [
            {
              id: "smoke-1",
              name: "Default Paper Agent",
              state: { kind: "idle" },
              has_llm_key: false,
            },
          ];
        case "recent_events":
          return [];
        default:
          throw new Error("smoke: unexpected command " + cmd);
      }
    };
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {
      invoke: fakeInvoke,
    };
    (window as unknown as { __TAURI__?: unknown }).__TAURI__ = {
      core: { invoke: fakeInvoke },
    };
  });
});

test("Fleet renders Default Paper Agent on dark background", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Default Paper Agent")).toBeVisible({ timeout: 10_000 });
  await expect(page.getByRole("button", { name: /create agent/i })).toBeVisible();

  const bg = await page.evaluate(() => {
    const probe = document.querySelector(".bg-neutral-950") as HTMLElement | null;
    if (!probe) return null;
    return getComputedStyle(probe).backgroundColor;
  });
  expect(bg).not.toBeNull();
  expect(bg).not.toBe("rgba(0, 0, 0, 0)");
  expect(bg).not.toMatch(/255,\s*255,\s*255/);
});
