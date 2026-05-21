import { test, expect } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  // Stub the Tauri bridge before any React code runs.
  await page.addInitScript(() => {
    const fakeInvoke = async (cmd: string, _args?: unknown) => {
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
    // @tauri-apps/api/core imports this shape internally.
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
  // Wait for Fleet content.
  await expect(page.getByText("Default Paper Agent")).toBeVisible({ timeout: 10_000 });
  await expect(page.getByRole("button", { name: /create agent/i })).toBeVisible();

  // Tailwind CSS rendering check — body or the root container should be dark.
  const bg = await page.evaluate(() => {
    const probe = document.querySelector(".bg-neutral-950") as HTMLElement | null;
    if (!probe) return null;
    return getComputedStyle(probe).backgroundColor;
  });
  expect(bg, "expected .bg-neutral-950 to resolve to a computed background-color").not.toBeNull();
  // rgb(...) form; neutral-950 in Tailwind 4 default palette resolves to a near-black colour.
  // We just assert it's not the unstyled default `rgba(0, 0, 0, 0)` / white.
  expect(bg).not.toBe("rgba(0, 0, 0, 0)");
  expect(bg).not.toMatch(/255,\s*255,\s*255/);
});
