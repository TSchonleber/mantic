import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import Fleet from "./Fleet";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("<Fleet />", () => {
  it("renders empty state when no agents", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "agent_list") return [];
      if (cmd === "recent_events") return [];
      throw new Error("unexpected: " + cmd);
    });
    render(<Fleet onOpenAccount={() => {}} />);
    expect(await screen.findByText(/no agents yet/i)).toBeInTheDocument();
  });

  it("renders one agent with state and arm button", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "agent_list") {
        return [
          {
            id: "a1",
            name: "Default Paper Agent",
            state: { kind: "idle" },
            has_llm_key: false,
          },
        ];
      }
      if (cmd === "recent_events") return [];
      throw new Error("unexpected: " + cmd);
    });
    render(<Fleet onOpenAccount={() => {}} />);
    expect(await screen.findByText(/Default Paper Agent/)).toBeInTheDocument();
    expect(screen.getByText(/Idle/)).toBeInTheDocument();
    expect(screen.getByText(/no LLM key set/i)).toBeInTheDocument();
  });

  it("opens Create modal when button clicked", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "agent_list") return [];
      if (cmd === "recent_events") return [];
      throw new Error("unexpected: " + cmd);
    });
    render(<Fleet onOpenAccount={() => {}} />);
    await waitFor(() => screen.getByRole("button", { name: /create agent/i }));
    fireEvent.click(screen.getByRole("button", { name: /create agent/i }));
    expect(await screen.findByText(/New agent/i)).toBeInTheDocument();
  });

  it("invokes agent_arm on Arm click", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "agent_list") {
        return [{ id: "a1", name: "A", state: { kind: "idle" }, has_llm_key: true }];
      }
      if (cmd === "recent_events") return [];
      if (cmd === "agent_arm") {
        return { id: "a1", name: "A", state: { kind: "armed" }, has_llm_key: true };
      }
      throw new Error("unexpected: " + cmd);
    });
    render(<Fleet onOpenAccount={() => {}} />);
    const button = await screen.findByRole("button", { name: /arm/i });
    fireEvent.click(button);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("agent_arm", { id: "a1" });
    });
  });
});
