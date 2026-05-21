import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import Wallet from "./Wallet";
import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

function renderWallet(onConnected = vi.fn()) {
  return render(
    <MemoryRouter>
      <Wallet onConnected={onConnected} initialStatus={null} />
    </MemoryRouter>,
  );
}

describe("<Wallet />", () => {
  it("shows Connect button when no wallet is connected", () => {
    renderWallet();
    expect(screen.getByRole("button", { name: /connect/i })).toBeInTheDocument();
  });

  it("invokes wallet_connect when the button is clicked", async () => {
    invokeMock.mockResolvedValueOnce({ url: "http://127.0.0.1:18421/connect?nonce=abc" });
    renderWallet();
    await userEvent.click(screen.getByRole("button", { name: /connect/i }));
    expect(invokeMock).toHaveBeenCalledWith("wallet_connect");
  });

  it("shows connected state when initialStatus is provided", () => {
    render(
      <MemoryRouter>
        <Wallet
          onConnected={vi.fn()}
          initialStatus={{
            session_pubkey_b58: "SessionPubkey",
            master_pubkey_b58: "MasterPubkey",
            authorization: {
              master_pubkey_b58: "MasterPubkey",
              session_pubkey_b58: "SessionPubkey",
              message: "x",
              signature_b58: "y",
              signed_at: "2026-05-21T12:00:00Z",
            },
          }}
        />
      </MemoryRouter>,
    );
    expect(screen.getByText(/SessionPubkey/i)).toBeInTheDocument();
    expect(screen.getByText(/MasterPubkey/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /revoke/i })).toBeInTheDocument();
  });

  it("invokes wallet_revoke when Revoke is clicked", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    render(
      <MemoryRouter>
        <Wallet
          onConnected={vi.fn()}
          initialStatus={{
            session_pubkey_b58: "S",
            master_pubkey_b58: "M",
            authorization: {
              master_pubkey_b58: "M",
              session_pubkey_b58: "S",
              message: "x",
              signature_b58: "y",
              signed_at: "2026-05-21T12:00:00Z",
            },
          }}
        />
      </MemoryRouter>,
    );
    await userEvent.click(screen.getByRole("button", { name: /revoke/i }));
    expect(invokeMock).toHaveBeenCalledWith("wallet_revoke");
  });
});
