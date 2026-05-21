import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import LiveTape from "./LiveTape";

const invokeMock = vi.mocked(invoke);

beforeEach(() => invokeMock.mockReset());

describe("<LiveTape />", () => {
  it("renders empty state when no events", async () => {
    invokeMock.mockResolvedValue([]);
    render(<LiveTape pollMs={9999} />);
    expect(await screen.findByText(/no events yet/i)).toBeInTheDocument();
  });

  it("renders event rows", async () => {
    invokeMock.mockResolvedValue([
      { id: 1, event_type: "decision", content: "thought one", created_at: 1700000000 },
      { id: 2, event_type: "result", content: "trade record", created_at: 1700000050 },
    ]);
    render(<LiveTape pollMs={9999} />);
    await waitFor(() => expect(screen.getByText(/thought one/i)).toBeInTheDocument());
    expect(screen.getByText(/trade record/i)).toBeInTheDocument();
    expect(screen.getByText(/decision/i)).toBeInTheDocument();
  });

  it("surfaces fetch errors", async () => {
    invokeMock.mockImplementation(async (cmd?: string) => {
      if (cmd === "recent_events") throw "brain unavailable";
      return undefined;
    });
    render(<LiveTape pollMs={9999} />);
    expect(await screen.findByText(/brain unavailable/i)).toBeInTheDocument();
  });
});
