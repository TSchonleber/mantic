import { useEffect, useState } from "react";
import {
  agentList,
  agentArm,
  agentPause,
  agentKill,
  agentFireTestSignal,
  AgentSummary,
  Signal,
} from "../lib/tauri-bridge";
import LiveTape from "../components/LiveTape";
import AgentConfigForm from "./AgentConfig";

function randomSignalId(): string {
  return Math.random().toString(16).slice(2, 18).padStart(16, "0");
}

function makeTestSignal(token: string): Signal {
  return {
    id: randomSignalId(),
    token_symbol: token,
    source: "test",
    context_tags: ["debug"],
    payload: { test: true },
  };
}

function describeState(state: AgentSummary["state"]): string {
  switch (state.kind) {
    case "idle":
      return "Idle";
    case "armed":
      return "Armed";
    case "running":
      return `Running (${state.step})`;
    case "paused":
      return "Paused";
    case "error":
      return `Error: ${state.reason}`;
  }
}

interface Props {
  onOpenAccount: () => void;
}

export default function Fleet({ onOpenAccount }: Props) {
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    try {
      setAgents(await agentList());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    refresh();
    const i = setInterval(refresh, 3000);
    return () => clearInterval(i);
  }, []);

  async function withRefresh(fn: () => Promise<unknown>) {
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="min-h-screen bg-neutral-950 p-8 text-neutral-100">
      <div className="mx-auto max-w-5xl space-y-8">
        <header className="flex items-center justify-between">
          <div>
            <h1 className="text-3xl font-semibold">Mantic</h1>
            <p className="mt-1 text-sm text-neutral-400">read the tape</p>
          </div>
          <button
            type="button"
            onClick={onOpenAccount}
            className="rounded-lg border border-neutral-700 px-3 py-1.5 text-sm text-neutral-300 hover:bg-neutral-800"
          >
            Account
          </button>
        </header>

        <section className="rounded-2xl bg-neutral-900 p-6 shadow-xl">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-medium">Fleet</h2>
            <button
              type="button"
              onClick={() => setCreating(true)}
              className="rounded-lg bg-blue-600 px-3 py-1.5 text-sm font-medium hover:bg-blue-500"
            >
              Create Agent
            </button>
          </div>
          {error && <p className="mt-3 text-sm text-red-400">{error}</p>}

          {agents.length === 0 ? (
            <p className="mt-4 text-sm text-neutral-500">
              No agents yet. The first-run default should appear shortly, or click Create Agent.
            </p>
          ) : (
            <ul className="mt-4 space-y-2">
              {agents.map((a) => (
                <li
                  key={a.id}
                  className="rounded-lg border border-neutral-800 bg-neutral-950 p-4"
                >
                  <div className="flex items-center justify-between">
                    <div>
                      <h3 className="font-medium">{a.name}</h3>
                      <p className="text-xs text-neutral-500">
                        id {a.id} · {describeState(a.state)}
                      </p>
                      {!a.has_llm_key && (
                        <p className="text-xs text-amber-400">no LLM key set</p>
                      )}
                    </div>
                    <div className="flex gap-2">
                      <button
                        type="button"
                        disabled={a.state.kind === "armed" || a.state.kind === "running"}
                        onClick={() => withRefresh(() => agentArm(a.id))}
                        className="rounded-lg bg-emerald-600 px-3 py-1.5 text-xs font-medium hover:bg-emerald-500 disabled:bg-neutral-700"
                      >
                        Arm
                      </button>
                      <button
                        type="button"
                        disabled={a.state.kind !== "armed" && a.state.kind !== "running"}
                        onClick={() => withRefresh(() => agentPause(a.id))}
                        className="rounded-lg bg-yellow-600 px-3 py-1.5 text-xs font-medium hover:bg-yellow-500 disabled:bg-neutral-700"
                      >
                        Pause
                      </button>
                      <button
                        type="button"
                        disabled={a.state.kind !== "armed"}
                        onClick={() => withRefresh(() => agentFireTestSignal(a.id, makeTestSignal("BONK")))}
                        className="rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium hover:bg-blue-500 disabled:bg-neutral-700"
                      >
                        Fire test signal
                      </button>
                      <button
                        type="button"
                        onClick={() => withRefresh(() => agentKill(a.id))}
                        className="rounded-lg border border-red-700 px-3 py-1.5 text-xs font-medium text-red-300 hover:bg-red-950"
                      >
                        Kill
                      </button>
                    </div>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>

        <LiveTape />

        {creating && (
          <AgentConfigForm
            onCancel={() => setCreating(false)}
            onCreated={async () => {
              setCreating(false);
              await refresh();
            }}
          />
        )}
      </div>
    </div>
  );
}
