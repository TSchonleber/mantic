import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import AgentConfigForm from "./AgentConfig";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("<AgentConfigForm />", () => {
  it("blocks submit when name is empty", async () => {
    const onCreated = vi.fn();
    render(<AgentConfigForm onCancel={() => {}} onCreated={onCreated} />);
    fireEvent.click(screen.getByRole("button", { name: /create/i }));
    await waitFor(() =>
      expect(invokeMock).not.toHaveBeenCalledWith("agent_create", expect.anything()),
    );
    expect(onCreated).not.toHaveBeenCalled();
  });

  it("invokes agent_create then agent_set_llm_key when key provided", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "agent_create") {
        return { id: "a1", name: "X", state: { kind: "idle" }, has_llm_key: false };
      }
      if (cmd === "agent_set_llm_key") return undefined;
      throw new Error("unexpected: " + cmd);
    });
    const onCreated = vi.fn();
    render(<AgentConfigForm onCancel={() => {}} onCreated={onCreated} />);

    fireEvent.change(screen.getByLabelText(/^name$/i), {
      target: { value: "Bonk Hunter" },
    });
    fireEvent.change(screen.getByLabelText(/anthropic api key/i), {
      target: { value: "sk-test" },
    });
    fireEvent.click(screen.getByRole("button", { name: /create/i }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("agent_create", {
        config: expect.objectContaining({ name: "Bonk Hunter" }),
      }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("agent_set_llm_key", {
        id: "a1",
        apiKey: "sk-test",
      }),
    );
    expect(onCreated).toHaveBeenCalled();
  });

  it("Cancel button calls onCancel", () => {
    const onCancel = vi.fn();
    render(<AgentConfigForm onCancel={onCancel} onCreated={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /cancel/i }));
    expect(onCancel).toHaveBeenCalled();
  });

  it("does not show API key field when mantic-proxy chosen", () => {
    render(<AgentConfigForm onCancel={() => {}} onCreated={() => {}} />);
    fireEvent.change(screen.getByLabelText(/llm backend/i), {
      target: { value: "mantic-proxy" },
    });
    expect(screen.queryByLabelText(/anthropic api key/i)).toBeNull();
  });
});
