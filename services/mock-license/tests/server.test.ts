import { describe, it, expect } from "vitest";
import { signLicenseJwt, verifyLicenseJwt, MOCK_SECRET } from "../src/jwt.js";

describe("license jwt", () => {
  it("signs a payload and verifies round-trip", async () => {
    const token = await signLicenseJwt({
      account_id: "acct_123",
      tier: "pro",
      expires_at: Math.floor(Date.now() / 1000) + 3600,
    });
    expect(typeof token).toBe("string");
    expect(token.split(".")).toHaveLength(3);

    const decoded = await verifyLicenseJwt(token);
    expect(decoded.account_id).toBe("acct_123");
    expect(decoded.tier).toBe("pro");
  });

  it("rejects a tampered token", async () => {
    const token = await signLicenseJwt({
      account_id: "acct_123",
      tier: "pro",
      expires_at: Math.floor(Date.now() / 1000) + 3600,
    });
    const tampered = token.slice(0, -4) + "XXXX";
    await expect(verifyLicenseJwt(tampered)).rejects.toThrow();
  });
});

import { app } from "../src/server.js";

describe("server /v1/pair", () => {
  it("issues a JWT for a valid pairing code", async () => {
    const res = await app.request("/v1/pair", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ code: "123456" }),
    });
    expect(res.status).toBe(200);
    const body = (await res.json()) as { token: string };
    expect(typeof body.token).toBe("string");
  });

  it("rejects an invalid code", async () => {
    const res = await app.request("/v1/pair", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ code: "" }),
    });
    expect(res.status).toBe(400);
  });
});

describe("server /v1/refresh", () => {
  it("issues a new JWT for a valid current token", async () => {
    const pairRes = await app.request("/v1/pair", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ code: "123456" }),
    });
    const { token } = (await pairRes.json()) as { token: string };

    const res = await app.request("/v1/refresh", {
      method: "POST",
      headers: { authorization: `Bearer ${token}` },
    });
    expect(res.status).toBe(200);
    const body = (await res.json()) as { token: string };
    expect(typeof body.token).toBe("string");
  });

  it("rejects missing auth", async () => {
    const res = await app.request("/v1/refresh", { method: "POST" });
    expect(res.status).toBe(401);
  });
});
