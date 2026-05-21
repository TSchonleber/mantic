import { SignJWT, jwtVerify } from "jose";

export interface LicensePayload {
  account_id: string;
  tier: "free" | "starter" | "pro" | "whale";
  expires_at: number;
}

export const MOCK_SECRET = new TextEncoder().encode(
  "dev-only-mantic-mock-secret-do-not-use-in-prod",
);

export async function signLicenseJwt(payload: LicensePayload): Promise<string> {
  return await new SignJWT({
    account_id: payload.account_id,
    tier: payload.tier,
  })
    .setProtectedHeader({ alg: "HS256" })
    .setIssuedAt()
    .setExpirationTime(payload.expires_at)
    .sign(MOCK_SECRET);
}

export async function verifyLicenseJwt(token: string): Promise<LicensePayload> {
  const { payload } = await jwtVerify(token, MOCK_SECRET);
  if (typeof payload.account_id !== "string" || typeof payload.tier !== "string") {
    throw new Error("invalid claims");
  }
  return {
    account_id: payload.account_id,
    tier: payload.tier as LicensePayload["tier"],
    expires_at: payload.exp ?? 0,
  };
}
