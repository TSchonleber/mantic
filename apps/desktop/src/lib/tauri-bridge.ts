import { invoke } from "@tauri-apps/api/core";

export interface AccountInfo {
  account_id: string;
  tier: "free" | "starter" | "pro" | "whale";
  expires_at: number;
}

export async function pairWithCode(code: string): Promise<AccountInfo> {
  return invoke<AccountInfo>("pair_with_code", { code });
}

export async function currentAccount(): Promise<AccountInfo | null> {
  try {
    return await invoke<AccountInfo>("current_account");
  } catch (e) {
    const msg = typeof e === "string" ? e : String(e);
    if (msg.includes("no license stored") || msg.includes("license expired")) {
      return null;
    }
    throw e;
  }
}

export async function signOut(): Promise<void> {
  await invoke("sign_out");
}
