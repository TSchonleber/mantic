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
    invokeMock.mockRejectedValueOnce("no license stored");
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    expect(await screen.findByText(/pair this device/i)).toBeInTheDocument();
  });

  it("shows Account route when a license is stored", async () => {
    invokeMock.mockResolvedValueOnce({
      account_id: "acct_pro",
      tier: "pro",
      expires_at: Math.floor(Date.now() / 1000) + 3600,
    });
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );
    await waitFor(() => {
      expect(screen.getByText(/acct_pro/i)).toBeInTheDocument();
    });
  });
});
