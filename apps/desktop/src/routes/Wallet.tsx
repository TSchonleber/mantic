import { useState } from "react";
import {
  WalletCredentials,
  walletConnect,
  walletRevoke,
} from "../lib/tauri-bridge";

interface Props {
  initialStatus: WalletCredentials | null;
  onConnected: (creds: WalletCredentials) => void;
}

export default function Wallet({ initialStatus, onConnected: _onConnected }: Props) {
  const [status, setStatus] = useState<WalletCredentials | null>(initialStatus);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [bridgeUrl, setBridgeUrl] = useState<string | null>(null);

  async function startConnect() {
    setError(null);
    setConnecting(true);
    try {
      const { url } = await walletConnect();
      setBridgeUrl(url);
    } catch (e) {
      const msg = typeof e === "string" ? e : (e as Error).message;
      setError(msg);
      setConnecting(false);
    }
  }

  async function revoke() {
    try {
      await walletRevoke();
      setStatus(null);
      setBridgeUrl(null);
    } catch (e) {
      const msg = typeof e === "string" ? e : (e as Error).message;
      setError(msg);
    }
  }

  if (status) {
    return (
      <div className="min-h-screen p-8">
        <div className="mx-auto max-w-2xl space-y-6">
          <header>
            <h1 className="text-3xl font-semibold">Wallet</h1>
            <p className="mt-1 text-sm text-neutral-400">Solana session wallet</p>
          </header>

          <section className="rounded-2xl bg-neutral-900 p-6 shadow-xl">
            <h2 className="text-lg font-medium">Connected</h2>
            <dl className="mt-4 space-y-3 text-sm">
              <div>
                <dt className="text-neutral-500">Session wallet</dt>
                <dd className="mt-1 break-all font-mono">{status.session_pubkey_b58}</dd>
              </div>
              <div>
                <dt className="text-neutral-500">Master wallet</dt>
                <dd className="mt-1 break-all font-mono">{status.master_pubkey_b58}</dd>
              </div>
              <div>
                <dt className="text-neutral-500">Authorized at</dt>
                <dd className="mt-1">{new Date(status.authorization.signed_at).toLocaleString()}</dd>
              </div>
            </dl>
          </section>

          {error && <div role="alert" className="rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300">{error}</div>}

          <div className="flex justify-end">
            <button
              type="button"
              onClick={revoke}
              className="rounded-lg border border-red-800 px-4 py-2 text-sm text-red-300 transition hover:bg-red-950"
            >
              Revoke wallet
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen items-center justify-center p-8">
      <div className="w-full max-w-md space-y-6 rounded-2xl bg-neutral-900 p-8 shadow-xl">
        <div>
          <h1 className="text-2xl font-semibold">Connect a Solana wallet</h1>
          <p className="mt-2 text-sm text-neutral-400">
            Mantic will generate a session wallet on your machine. You'll authorize it by
            signing a message in your master wallet, then fund it manually when you're
            ready to trade.
          </p>
        </div>

        <button
          type="button"
          disabled={connecting}
          onClick={startConnect}
          className="w-full rounded-lg bg-blue-600 px-4 py-2 font-medium transition hover:bg-blue-500 disabled:cursor-not-allowed disabled:bg-neutral-700"
        >
          {connecting ? "Waiting for browser..." : "Connect Phantom or Solflare"}
        </button>

        {bridgeUrl && (
          <div className="rounded-lg bg-neutral-950 p-3 text-xs text-neutral-400">
            Your browser should have opened. If not:{" "}
            <a href={bridgeUrl} className="text-blue-400 underline">{bridgeUrl}</a>
          </div>
        )}

        {error && (
          <div role="alert" className="rounded-lg bg-red-950 px-3 py-2 text-sm text-red-300">{error}</div>
        )}
      </div>
    </div>
  );
}
