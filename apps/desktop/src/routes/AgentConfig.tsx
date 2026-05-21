import { useState } from "react";
import {
  AgentConfigInput,
  LlmBackendKind,
  agentCreate,
  agentSetLlmKey,
} from "../lib/tauri-bridge";

interface Props {
  onCancel: () => void;
  onCreated: () => void;
}

const DEFAULT_CONFIG: AgentConfigInput = {
  name: "",
  max_position_sol: 0.5,
  daily_loss_cap_sol: 2.0,
  nl_overlay: "",
  strategy_template_id: "mock",
  llm_backend: "anthropic-direct",
  llm_model: "claude-sonnet-4-6",
  max_tokens: 1024,
  temperature: 0.0,
};

export default function AgentConfigForm({ onCancel, onCreated }: Props) {
  const [form, setForm] = useState<AgentConfigInput>(DEFAULT_CONFIG);
  const [apiKey, setApiKey] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function update<K extends keyof AgentConfigInput>(key: K, value: AgentConfigInput[K]) {
    setForm((current) => ({ ...current, [key]: value }));
  }

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    if (!form.name.trim()) {
      setError("Name is required");
      return;
    }
    if (form.max_position_sol <= 0) {
      setError("Max position size must be > 0");
      return;
    }

    setSubmitting(true);
    setError(null);
    try {
      const summary = await agentCreate(form);
      if (form.llm_backend === "anthropic-direct" && apiKey.trim()) {
        await agentSetLlmKey(summary.id, apiKey.trim());
      }
      onCreated();
    } catch (err) {
      setError(String(err));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-black/60 p-6">
      <form
        onSubmit={submit}
        className="w-full max-w-xl space-y-4 rounded-2xl bg-neutral-900 p-6 shadow-xl"
      >
        <h2 className="text-lg font-medium">New agent</h2>
        {error && <p className="text-sm text-red-400">{error}</p>}

        <label className="block">
          <span className="text-sm text-neutral-300">Name</span>
          <input
            type="text"
            value={form.name}
            onChange={(event) => update("name", event.target.value)}
            className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
            required
          />
        </label>

        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <label className="block">
            <span className="text-sm text-neutral-300">Max position (SOL)</span>
            <input
              type="number"
              step="0.01"
              min="0.01"
              value={form.max_position_sol}
              onChange={(event) =>
                update("max_position_sol", Number.parseFloat(event.target.value))
              }
              className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
            />
          </label>
          <label className="block">
            <span className="text-sm text-neutral-300">Daily loss cap (SOL)</span>
            <input
              type="number"
              step="0.1"
              min="0"
              value={form.daily_loss_cap_sol}
              onChange={(event) =>
                update("daily_loss_cap_sol", Number.parseFloat(event.target.value))
              }
              className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
            />
          </label>
        </div>

        <label className="block">
          <span className="text-sm text-neutral-300">Behavior notes</span>
          <textarea
            value={form.nl_overlay}
            onChange={(event) => update("nl_overlay", event.target.value)}
            rows={3}
            className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
          />
        </label>

        <label className="block">
          <span className="text-sm text-neutral-300">LLM backend</span>
          <select
            value={form.llm_backend}
            onChange={(event) => update("llm_backend", event.target.value as LlmBackendKind)}
            className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
          >
            <option value="anthropic-direct">Anthropic (BYO key)</option>
            <option value="mantic-proxy">Mantic proxy</option>
          </select>
        </label>

        {form.llm_backend === "anthropic-direct" && (
          <label className="block">
            <span className="text-sm text-neutral-300">Anthropic API key</span>
            <input
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 font-mono text-sm"
              autoComplete="off"
            />
          </label>
        )}

        <label className="block">
          <span className="text-sm text-neutral-300">Strategy template</span>
          <select
            value={form.strategy_template_id}
            onChange={(event) => update("strategy_template_id", event.target.value)}
            className="mt-1 w-full rounded-lg bg-neutral-950 px-3 py-2 text-sm"
          >
            <option value="mock">Mock</option>
          </select>
        </label>

        <footer className="flex justify-end gap-2 pt-2">
          <button
            type="button"
            onClick={onCancel}
            className="rounded-lg border border-neutral-700 px-3 py-1.5 text-sm hover:bg-neutral-800"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={submitting}
            className="rounded-lg bg-blue-600 px-3 py-1.5 text-sm font-medium hover:bg-blue-500 disabled:bg-neutral-700"
          >
            {submitting ? "Creating..." : "Create"}
          </button>
        </footer>
      </form>
    </div>
  );
}
