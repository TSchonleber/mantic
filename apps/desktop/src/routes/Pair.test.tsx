import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import Pair from "./Pair";
import { invoke } from "@tauri-apps/api/core";

const invokeMock = vi.mocked(invoke);

function renderPair(onPaired = vi.fn()) {
  return render(
    <MemoryRouter>
      <Pair onPaired={onPaired} />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("<Pair />", () => {
  it("renders a pairing code input and submit button", () => {
    renderPair();
    expect(screen.getByLabelText(/pairing code/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /pair device/i })).toBeInTheDocument();
  });

  it("submits the code and calls onPaired with the account info", async () => {
    const onPaired = vi.fn();
    invokeMock.mockResolvedValueOnce({
      account_id: "acct_99",
      tier: "starter",
      expires_at: 9999999999,
    });

    renderPair(onPaired);
    await userEvent.type(screen.getByLabelText(/pairing code/i), "123456");
    await userEvent.click(screen.getByRole("button", { name: /pair device/i }));

    expect(invokeMock).toHaveBeenCalledWith("pair_with_code", { code: "123456" });
    expect(onPaired).toHaveBeenCalledWith(
      expect.objectContaining({ account_id: "acct_99", tier: "starter" }),
    );
  });

  it("shows an error message when pairing fails", async () => {
    invokeMock.mockRejectedValueOnce("invalid pairing code");

    renderPair();
    await userEvent.type(screen.getByLabelText(/pairing code/i), "x");
    await userEvent.click(screen.getByRole("button", { name: /pair device/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/invalid pairing code/i);
  });
});
