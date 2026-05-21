import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import Account from "./Account";
import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

const account = {
  account_id: "acct_42",
  tier: "pro" as const,
  expires_at: Math.floor(Date.now() / 1000) + 86400,
};

beforeEach(() => {
  invokeMock.mockReset();
});

describe("<Account />", () => {
  it("renders tier and expiry", () => {
    render(
      <MemoryRouter>
        <Account account={account} onSignOut={vi.fn()} />
      </MemoryRouter>,
    );
    expect(screen.getByText(/pro/i)).toBeInTheDocument();
    expect(screen.getByText(/acct_42/i)).toBeInTheDocument();
    expect(screen.getByText(/license expires/i)).toBeInTheDocument();
  });

  it("invokes sign_out and calls onSignOut on click", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    const onSignOut = vi.fn();
    render(
      <MemoryRouter>
        <Account account={account} onSignOut={onSignOut} />
      </MemoryRouter>,
    );
    await userEvent.click(screen.getByRole("button", { name: /unpair this device/i }));
    expect(invokeMock).toHaveBeenCalledWith("sign_out");
    expect(onSignOut).toHaveBeenCalled();
  });
});
