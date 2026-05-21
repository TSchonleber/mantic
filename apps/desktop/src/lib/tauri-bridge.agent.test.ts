import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  agentArm,
  agentCreate,
  agentFireTestSignal,
  agentGet,
  agentKill,
  agentList,
  agentPause,
  agentSetLlmKey,
} from "./tauri-bridge";

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
});

describe("agent bridge", () => {
  it("agentCreate passes config", async () => {
    invokeMock.mockResolvedValueOnce({
      id: "1",
      name: "x",
      state: { kind: "idle" },
      has_llm_key: false,
    });
    const r = await agentCreate({
      name: "x",
      max_position_sol: 0.5,
      daily_loss_cap_sol: 2,
      nl_overlay: "",
      strategy_template_id: "mock",
      llm_backend: "anthropic-direct",
      llm_model: "claude-sonnet-4-6",
      max_tokens: 1024,
      temperature: 0,
    });
    expect(invokeMock).toHaveBeenCalledWith("agent_create", {
      config: expect.objectContaining({ name: "x" }),
    });
    expect(r.id).toBe("1");
  });

  it("agentList returns array", async () => {
    invokeMock.mockResolvedValueOnce([]);
    expect(await agentList()).toEqual([]);
    expect(invokeMock).toHaveBeenCalledWith("agent_list");
  });

  it("agentGet uses id", async () => {
    invokeMock.mockResolvedValueOnce({
      config: {},
      state: { kind: "idle" },
      has_llm_key: false,
      open_position_ids: [],
    });
    await agentGet("a1");
    expect(invokeMock).toHaveBeenCalledWith("agent_get", { id: "a1" });
  });

  it("agentArm/pause/kill use id", async () => {
    invokeMock.mockResolvedValue({ id: "a1", name: "x", state: { kind: "armed" }, has_llm_key: false });
    await agentArm("a1");
    expect(invokeMock).toHaveBeenLastCalledWith("agent_arm", { id: "a1" });
    await agentPause("a1");
    expect(invokeMock).toHaveBeenLastCalledWith("agent_pause", { id: "a1" });
    invokeMock.mockResolvedValueOnce(undefined);
    await agentKill("a1");
    expect(invokeMock).toHaveBeenLastCalledWith("agent_kill", { id: "a1" });
  });

  it("agentFireTestSignal passes signal", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await agentFireTestSignal("a1", {
      id: "sig1",
      token_symbol: "BONK",
      source: "test",
      context_tags: [],
      payload: {},
    });
    expect(invokeMock).toHaveBeenCalledWith("agent_fire_test_signal", {
      id: "a1",
      signal: expect.objectContaining({ token_symbol: "BONK" }),
    });
  });

  it("agentSetLlmKey passes apiKey", async () => {
    invokeMock.mockResolvedValueOnce(undefined);
    await agentSetLlmKey("a1", "sk-xxx");
    expect(invokeMock).toHaveBeenCalledWith("agent_set_llm_key", {
      id: "a1",
      apiKey: "sk-xxx",
    });
  });
});
