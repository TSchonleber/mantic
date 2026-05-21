import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { pairWithCode, currentAccount, signOut } from "./tauri-bridge";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("tauri-bridge", () => {
  it("pairWithCode invokes the rust command with the code", async () => {
    invokeMock.mockResolvedValueOnce({
      account_id: "acct_1",
      tier: "pro",
      expires_at: 9999999999,
    });
    const result = await pairWithCode("123456");
    expect(invokeMock).toHaveBeenCalledWith("pair_with_code", { code: "123456" });
    expect(result.tier).toBe("pro");
  });

  it("currentAccount returns null when no license is stored", async () => {
    invokeMock.mockRejectedValueOnce("no license stored");
    const result = await currentAccount();
    expect(result).toBeNull();
  });

  it("currentAccount returns account info when stored", async () => {
    invokeMock.mockResolvedValueOnce({
      account_id: "acct_1",
      tier: "pro",
      expires_at: 9999999999,
    });
    const result = await currentAccount();
    expect(result?.account_id).toBe("acct_1");
  });

  it("signOut invokes sign_out", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await signOut();
    expect(invokeMock).toHaveBeenCalledWith("sign_out");
  });
});
