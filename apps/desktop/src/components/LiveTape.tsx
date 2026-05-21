import { useEffect, useState } from "react";
import { recentEvents, EventSummary } from "../lib/tauri-bridge";

interface Props {
  pollMs?: number;
  limit?: number;
}

export default function LiveTape({ pollMs = 5000, limit = 50 }: Props) {
  const [events, setEvents] = useState<EventSummary[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function tick() {
      try {
        const e = await recentEvents(limit);
        if (!cancelled) setEvents(e);
      } catch (err) {
        if (!cancelled) setError(String(err));
      }
    }
    tick();
    const handle = setInterval(tick, pollMs);
    return () => {
      cancelled = true;
      clearInterval(handle);
    };
  }, [pollMs, limit]);

  return (
    <section className="rounded-2xl bg-neutral-900 p-6 shadow-xl">
      <header className="flex items-center justify-between">
        <h2 className="text-lg font-medium">Live tape</h2>
        <span className="text-xs text-neutral-500">
          {events.length} event{events.length === 1 ? "" : "s"}
        </span>
      </header>
      {error && <p className="mt-3 text-sm text-red-400">{error}</p>}
      <ol className="mt-4 space-y-2">
        {events.length === 0 && !error && (
          <li className="text-sm text-neutral-500">
            No events yet. Fire a test signal to populate the tape.
          </li>
        )}
        {events.map((e) => (
          <li
            key={e.id}
            className="rounded-lg border border-neutral-800 bg-neutral-950 p-3 text-xs"
          >
            <div className="flex items-baseline justify-between">
              <span className="font-mono uppercase text-neutral-400">
                {e.event_type}
              </span>
              <time className="text-neutral-600">
                {new Date(e.created_at * 1000).toLocaleTimeString()}
              </time>
            </div>
            <pre className="mt-1 whitespace-pre-wrap break-words text-neutral-200">
              {e.content}
            </pre>
          </li>
        ))}
      </ol>
    </section>
  );
}
