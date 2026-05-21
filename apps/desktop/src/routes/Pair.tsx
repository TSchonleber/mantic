import { useState, FormEvent } from "react";
import { pairWithCode, AccountInfo } from "../lib/tauri-bridge";

interface Props {
  onPaired: (info: AccountInfo) => void;
}

export default function Pair({ onPaired }: Props) {
  const [code, setCode] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const info = await pairWithCode(code.trim());
      onPaired(info);
    } catch (err) {
      const msg = typeof err === "string" ? err : (err as Error).message;
      setError(msg);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center p-8">
      <form
        onSubmit={onSubmit}
        className="w-full max-w-md space-y-6 rounded-2xl bg-neutral-900 p-8 shadow-xl"
      >
        <div>
          <h1 className="text-2xl font-semibold">Pair this device</h1>
          <p className="mt-2 text-sm text-neutral-400">
            Enter the 6-digit pairing code from brainctl.org to link this machine to your
            Mantic account.
          </p>
        </div>

        <label className="block">
          <span className="text-sm font-medium text-neutral-300">Pairing code</span>
          <input
            type="text"
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder="123456"
            autoComplete="one-time-code"
            inputMode="numeric"
            className="mt-1 w-full rounded-lg border border-neutral-700 bg-neutral-800 px-3 py-2 font-mono text-lg tracking-widest focus:border-blue-500 focus:outline-none"
            disabled={submitting}
          />
        </label>

        {error && (
          <div role="alert" className="rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300">
            {error}
          </div>
        )}

        <button
          type="submit"
          disabled={submitting || code.trim().length === 0}
          className="w-full rounded-lg bg-blue-600 px-4 py-2 font-medium transition hover:bg-blue-500 disabled:cursor-not-allowed disabled:bg-neutral-700"
        >
          {submitting ? "Pairing..." : "Pair device"}
        </button>
      </form>
    </div>
  );
}
