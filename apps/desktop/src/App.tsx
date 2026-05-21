import { useEffect, useState } from "react";
import {
  AccountInfo,
  WalletCredentials,
  currentAccount,
  walletStatus,
} from "./lib/tauri-bridge";
import Pair from "./routes/Pair";
import Wallet from "./routes/Wallet";
import Account from "./routes/Account";

type Status = "loading" | "unpaired" | "paired-no-wallet" | "paired-with-wallet";

export default function App() {
  const [status, setStatus] = useState<Status>("loading");
  const [account, setAccount] = useState<AccountInfo | null>(null);
  const [wallet, setWallet] = useState<WalletCredentials | null>(null);

  useEffect(() => {
    let active = true;
    (async () => {
      const acct = await currentAccount();
      if (!active) return;
      if (!acct) {
        setStatus("unpaired");
        return;
      }
      setAccount(acct);
      const w = await walletStatus();
      if (!active) return;
      if (w) {
        setWallet(w);
        setStatus("paired-with-wallet");
      } else {
        setStatus("paired-no-wallet");
      }
    })().catch(() => {
      if (active) setStatus("unpaired");
    });
    return () => {
      active = false;
    };
  }, []);

  if (status === "loading") {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <p className="text-neutral-500">Loading…</p>
      </div>
    );
  }

  if (status === "unpaired" || !account) {
    return (
      <Pair
        onPaired={async (info) => {
          setAccount(info);
          const w = await walletStatus();
          if (w) {
            setWallet(w);
            setStatus("paired-with-wallet");
          } else {
            setStatus("paired-no-wallet");
          }
        }}
      />
    );
  }

  if (status === "paired-no-wallet" || !wallet) {
    return (
      <Wallet
        initialStatus={wallet}
        onConnected={(creds) => {
          setWallet(creds);
          setStatus("paired-with-wallet");
        }}
      />
    );
  }

  return (
    <Account
      account={account}
      onSignOut={() => {
        setAccount(null);
        setWallet(null);
        setStatus("unpaired");
      }}
    />
  );
}
