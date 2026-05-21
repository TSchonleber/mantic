import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { pairWithCode, currentAccount, signOut } from "./tauri-bridge";
import {
  brainStatus,
  recentEvents,
  recentMemories,
  memoryAdd,
  eventAdd,
  decisionAdd,
  entityCreate,
  entityObserve,
  agentRegister,
  agentWrapUp,
  agentOrient,
  memorySearch,
} from "./tauri-bridge";

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

describe("brain bridge — reads", () => {
  it("brainStatus invokes the rust command", async () => {
    invokeMock.mockResolvedValueOnce({ memory_count: 5, event_count: 12 });
    const r = await brainStatus();
    expect(invokeMock).toHaveBeenCalledWith("brain_status");
    expect(r.memory_count).toBe(5);
    expect(r.event_count).toBe(12);
  });

  it("recentEvents passes the limit", async () => {
    invokeMock.mockResolvedValueOnce([]);
    await recentEvents(10);
    expect(invokeMock).toHaveBeenCalledWith("recent_events", { limit: 10 });
  });

  it("recentMemories passes the limit", async () => {
    invokeMock.mockResolvedValueOnce([]);
    await recentMemories(7);
    expect(invokeMock).toHaveBeenCalledWith("recent_memories", { limit: 7 });
  });
});

describe("brain bridge — writes", () => {
  it("memoryAdd passes content + category", async () => {
    invokeMock.mockResolvedValueOnce({ memory_id: 1 });
    await memoryAdd({ content: "hi", category: "lesson" });
    expect(invokeMock).toHaveBeenCalledWith("memory_add", {
      content: "hi",
      category: "lesson",
      scope: undefined,
      tags: undefined,
    });
  });

  it("memoryAdd passes optional scope and tags", async () => {
    invokeMock.mockResolvedValueOnce({});
    await memoryAdd({
      content: "hi",
      category: "lesson",
      scope: "project:mantic",
      tags: "trading,llm",
    });
    expect(invokeMock).toHaveBeenCalledWith("memory_add", {
      content: "hi",
      category: "lesson",
      scope: "project:mantic",
      tags: "trading,llm",
    });
  });

  it("decisionAdd passes title + rationale", async () => {
    invokeMock.mockResolvedValueOnce({ decision_id: 9 });
    await decisionAdd({ title: "x", rationale: "y" });
    expect(invokeMock).toHaveBeenCalledWith("decision_add", {
      title: "x",
      rationale: "y",
      project: undefined,
    });
  });

  it("agentRegister passes id + name", async () => {
    invokeMock.mockResolvedValueOnce({ ok: true });
    await agentRegister({ id: "a", name: "A" });
    expect(invokeMock).toHaveBeenCalledWith("agent_register", {
      id: "a",
      name: "A",
      agentType: undefined,
    });
  });
});

describe("brain bridge — complex reads", () => {
  it("memorySearch passes the query", async () => {
    invokeMock.mockResolvedValueOnce({ hits: [] });
    await memorySearch({ query: "trade", limit: 5 });
    expect(invokeMock).toHaveBeenCalledWith("memory_search", { query: "trade", limit: 5 });
  });

  it("agentOrient invokes correctly", async () => {
    invokeMock.mockResolvedValueOnce({ memories: [], events: [] });
    await agentOrient({ agentId: "claude-code-test", project: "mantic" });
    expect(invokeMock).toHaveBeenCalledWith("agent_orient", {
      agentId: "claude-code-test",
      project: "mantic",
      query: undefined,
    });
  });
});

describe("eventAdd and remaining bridges", () => {
  it("eventAdd passes type/content/importance", async () => {
    invokeMock.mockResolvedValueOnce({});
    await eventAdd({ eventType: "result", content: "x", importance: 0.8 });
    expect(invokeMock).toHaveBeenCalledWith("event_add", {
      eventType: "result",
      content: "x",
      importance: 0.8,
    });
  });

  it("entityCreate passes name/type/scope", async () => {
    invokeMock.mockResolvedValueOnce({ entity_id: 1 });
    await entityCreate({ name: "BONK", entityType: "token", scope: "project:mantic" });
    expect(invokeMock).toHaveBeenCalledWith("entity_create", {
      name: "BONK",
      entityType: "token",
      scope: "project:mantic",
    });
  });

  it("entityObserve passes id and text", async () => {
    invokeMock.mockResolvedValueOnce({});
    await entityObserve({ entityId: 42, observation: "rugged" });
    expect(invokeMock).toHaveBeenCalledWith("entity_observe", {
      entityId: 42,
      observation: "rugged",
    });
  });

  it("agentWrapUp passes required + optional fields", async () => {
    invokeMock.mockResolvedValueOnce({});
    await agentWrapUp({
      agentId: "a",
      summary: "done",
      goal: "ship",
      openLoops: "none",
      nextStep: "next",
      project: "mantic",
    });
    expect(invokeMock).toHaveBeenCalledWith("agent_wrap_up", {
      agentId: "a",
      summary: "done",
      goal: "ship",
      openLoops: "none",
      nextStep: "next",
      project: "mantic",
    });
  });
});
