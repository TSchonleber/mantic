import { useEffect, useState } from "react";
import {
  AgentSummary,
  Signal,
  agentArm,
  agentFireTestSignal,
  agentKill,
  agentList,
  agentPause,
} from "../lib/tauri-bridge";
import LiveTape from "../components/LiveTape";
import AgentConfigForm from "./AgentConfig";

interface Props {
  onOpenAccount: () => void;
}

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

export default function Fleet({ onOpenAccount }: Props) {
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    try {
      setAgents(await agentList());
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }

  useEffect(() => {
    refresh();
    const handle = setInterval(refresh, 3000);
    return () => clearInterval(handle);
  }, []);

  async function withRefresh(action: () => Promise<unknown>) {
    try {
      await action();
      await refresh();
    } catch (err) {
      setError(String(err));
    }
  }

  return (
    <div className="min-h-screen bg-neutral-950 p-6 text-neutral-100 sm:p-8">
      <div className="mx-auto max-w-5xl space-y-8">
        <header className="flex items-center justify-between gap-4">
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
          <div className="flex items-center justify-between gap-4">
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
            <p className="mt-4 text-sm text-neutral-500">No agents yet.</p>
          ) : (
            <ul className="mt-4 space-y-2">
              {agents.map((agent) => (
                <li
                  key={agent.id}
                  className="rounded-lg border border-neutral-800 bg-neutral-950 p-4"
                >
                  <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
                    <div>
                      <h3 className="font-medium">{agent.name}</h3>
                      <p className="mt-1 text-xs text-neutral-500">
                        id {agent.id} - {describeState(agent.state)}
                      </p>
                      {!agent.has_llm_key && (
                        <p className="mt-1 text-xs text-amber-400">no LLM key set</p>
                      )}
                    </div>
                    <div className="flex flex-wrap gap-2">
                      <button
                        type="button"
                        disabled={agent.state.kind === "armed" || agent.state.kind === "running"}
                        onClick={() => withRefresh(() => agentArm(agent.id))}
                        className="rounded-lg bg-emerald-600 px-3 py-1.5 text-xs font-medium hover:bg-emerald-500 disabled:bg-neutral-700"
                      >
                        Arm
                      </button>
                      <button
                        type="button"
                        disabled={agent.state.kind !== "armed" && agent.state.kind !== "running"}
                        onClick={() => withRefresh(() => agentPause(agent.id))}
                        className="rounded-lg bg-yellow-600 px-3 py-1.5 text-xs font-medium hover:bg-yellow-500 disabled:bg-neutral-700"
                      >
                        Pause
                      </button>
                      <button
                        type="button"
                        disabled={agent.state.kind !== "armed"}
                        onClick={() =>
                          withRefresh(() =>
                            agentFireTestSignal(agent.id, makeTestSignal("BONK")),
                          )
                        }
                        className="rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium hover:bg-blue-500 disabled:bg-neutral-700"
                      >
                        Fire test signal
                      </button>
                      <button
                        type="button"
                        onClick={() => withRefresh(() => agentKill(agent.id))}
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
