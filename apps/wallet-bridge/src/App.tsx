import { useEffect, useState } from "react";
import { useWallet } from "@solana/wallet-adapter-react";
import { WalletMultiButton } from "@solana/wallet-adapter-react-ui";
import bs58 from "bs58";

type Status = "idle" | "signing" | "submitting" | "done" | "error";

export default function App() {
  const { publicKey, signMessage, connected, wallet } = useWallet();
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState<string | null>(null);
  const [authMessage, setAuthMessage] = useState<string | null>(null);
  const [nonce, setNonce] = useState<string | null>(null);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    setNonce(params.get("nonce"));
    fetch(`/auth-message${window.location.search}`)
      .then((r) => r.text())
      .then(setAuthMessage)
      .catch((e) => setError(`Failed to fetch auth message: ${e}`));
  }, []);

  async function authorize() {
    if (!publicKey || !signMessage || !authMessage || !nonce) return;
    setStatus("signing");
    setError(null);
    try {
      const messageBytes = new TextEncoder().encode(authMessage);
      const signature = await signMessage(messageBytes);
      setStatus("submitting");
      const res = await fetch("/authorize", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          pubkey: publicKey.toBase58(),
          signature: bs58.encode(signature),
          message: authMessage,
          nonce,
        }),
      });
      if (!res.ok) {
        const body = await res.text();
        throw new Error(`Authorize failed (${res.status}): ${body}`);
      }
      setStatus("done");
    } catch (e) {
      setStatus("error");
      setError((e as Error).message);
    }
  }

  return (
    <div className="card">
      <h1>Connect wallet to Mantic</h1>
      <p>
        Mantic will create a session wallet on your machine. You'll authorize it by signing a
        message in your Solana wallet, then fund it manually when you're ready to trade.
      </p>

      {!connected && (
        <div style={{ marginTop: 20 }}>
          <WalletMultiButton />
        </div>
      )}

      {connected && authMessage && (
        <>
          <p style={{ marginTop: 20 }}>Connected to {wallet?.adapter.name}. Sign this to authorize:</p>
          <pre>{authMessage}</pre>
          <button
            disabled={status === "signing" || status === "submitting" || status === "done"}
            onClick={authorize}
          >
            {status === "signing" && "Sign in your wallet..."}
            {status === "submitting" && "Authorizing..."}
            {status === "done" && "Done — you can close this tab"}
            {status === "idle" && "Authorize"}
            {status === "error" && "Retry"}
          </button>
          {status === "done" && <div className="ok">Authorization complete. Return to Mantic.</div>}
          {error && <div className="err">{error}</div>}
        </>
      )}
    </div>
  );
}
