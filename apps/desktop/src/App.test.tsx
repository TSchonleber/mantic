import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import App from "./App";
import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("<App />", () => {
  it("shows Pair route when no license is stored", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "current_account") throw "no license stored";
      throw new Error(`unexpected: ${cmd}`);
    });
    render(<MemoryRouter><App /></MemoryRouter>);
    expect(await screen.findByText(/pair this device/i)).toBeInTheDocument();
  });

  it("shows Wallet route when paired but no wallet", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "current_account") {
        return { account_id: "acct_x", tier: "pro", expires_at: Math.floor(Date.now() / 1000) + 3600 };
      }
      if (cmd === "wallet_status") return null;
      throw new Error(`unexpected: ${cmd}`);
    });
    render(<MemoryRouter><App /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText(/connect a solana wallet/i)).toBeInTheDocument();
    });
  });

  it("shows Account route when paired AND wallet connected", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "current_account") {
        return { account_id: "acct_x", tier: "pro", expires_at: Math.floor(Date.now() / 1000) + 3600 };
      }
      if (cmd === "wallet_status") {
        return {
          session_pubkey_b58: "Sess",
          master_pubkey_b58: "Mast",
          authorization: {
            master_pubkey_b58: "Mast",
            session_pubkey_b58: "Sess",
            message: "x",
            signature_b58: "y",
            signed_at: "2026-05-21T12:00:00Z",
          },
        };
      }
      throw new Error(`unexpected: ${cmd}`);
    });
    render(<MemoryRouter><App /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText(/acct_x/i)).toBeInTheDocument();
    });
  });
});
