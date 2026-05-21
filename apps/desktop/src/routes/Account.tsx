import { signOut, AccountInfo } from "../lib/tauri-bridge";

interface Props {
  account: AccountInfo;
  onSignOut: () => void;
}

function formatExpiry(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString();
}

export default function Account({ account, onSignOut }: Props) {
  async function handleSignOut() {
    await signOut();
    onSignOut();
  }

  return (
    <div className="min-h-screen p-8">
      <div className="mx-auto max-w-2xl space-y-8">
        <header>
          <h1 className="text-3xl font-semibold">Mantic</h1>
          <p className="mt-1 text-sm text-neutral-400">read the tape</p>
        </header>

        <section className="rounded-2xl bg-neutral-900 p-6 shadow-xl">
          <h2 className="text-lg font-medium">Account</h2>
          <dl className="mt-4 grid grid-cols-2 gap-4 text-sm">
            <div>
              <dt className="text-neutral-500">Account ID</dt>
              <dd className="mt-1 font-mono">{account.account_id}</dd>
            </div>
            <div>
              <dt className="text-neutral-500">Tier</dt>
              <dd className="mt-1 font-medium uppercase">{account.tier}</dd>
            </div>
            <div className="col-span-2">
              <dt className="text-neutral-500">License expires</dt>
              <dd className="mt-1">{formatExpiry(account.expires_at)}</dd>
            </div>
          </dl>
        </section>

        <section className="rounded-2xl bg-neutral-900 p-6 text-sm text-neutral-400 shadow-xl">
          <p>
            The trading UI ships in a later milestone. This shell verifies your account is paired
            and the license is healthy.
          </p>
        </section>

        <div className="flex justify-end">
          <button
            type="button"
            onClick={handleSignOut}
            className="rounded-lg border border-neutral-700 px-4 py-2 text-sm text-neutral-300 transition hover:bg-neutral-800"
          >
            Unpair this device
          </button>
        </div>
      </div>
    </div>
  );
}
