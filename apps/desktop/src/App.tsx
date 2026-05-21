import { useEffect, useState } from "react";
import { currentAccount, AccountInfo } from "./lib/tauri-bridge";
import Pair from "./routes/Pair";
import Account from "./routes/Account";

type Status = "loading" | "unpaired" | "paired";

export default function App() {
  const [status, setStatus] = useState<Status>("loading");
  const [account, setAccount] = useState<AccountInfo | null>(null);

  useEffect(() => {
    let active = true;
    (async () => {
      const info = await currentAccount();
      if (!active) return;
      if (info) {
        setAccount(info);
        setStatus("paired");
      } else {
        setStatus("unpaired");
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
        onPaired={(info) => {
          setAccount(info);
          setStatus("paired");
        }}
      />
    );
  }

  return (
    <Account
      account={account}
      onSignOut={() => {
        setAccount(null);
        setStatus("unpaired");
      }}
    />
  );
}
