import { Hono } from "hono";
import { serve } from "@hono/node-server";
import { signLicenseJwt, verifyLicenseJwt } from "./jwt.js";

export const app = new Hono();

app.post("/v1/pair", async (c) => {
  const body = (await c.req.json().catch(() => ({}))) as { code?: string };
  if (!body.code || body.code.length < 4) {
    return c.json({ error: "invalid_code" }, 400);
  }
  const token = await signLicenseJwt({
    account_id: "acct_dev_" + body.code,
    tier: "pro",
    expires_at: Math.floor(Date.now() / 1000) + 60 * 60 * 24,
  });
  return c.json({ token });
});

app.post("/v1/refresh", async (c) => {
  const auth = c.req.header("authorization");
  if (!auth?.startsWith("Bearer ")) {
    return c.json({ error: "missing_auth" }, 401);
  }
  const current = auth.slice("Bearer ".length);
  try {
    const decoded = await verifyLicenseJwt(current);
    const token = await signLicenseJwt({
      account_id: decoded.account_id,
      tier: decoded.tier,
      expires_at: Math.floor(Date.now() / 1000) + 60 * 60 * 24,
    });
    return c.json({ token });
  } catch {
    return c.json({ error: "invalid_token" }, 401);
  }
});

app.get("/healthz", (c) => c.json({ ok: true }));

const port = Number(process.env.PORT ?? 4001);

if (import.meta.url === `file://${process.argv[1]}`) {
  serve({ fetch: app.fetch, port }, (info) => {
    console.log(`[mock-license] listening on http://localhost:${info.port}`);
  });
}
