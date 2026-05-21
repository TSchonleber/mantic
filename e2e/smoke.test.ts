import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { spawn, ChildProcess, execSync } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { platform } from "node:os";

function findBinary(): string {
  const repoRoot = resolve(import.meta.dirname, "..");
  const base = resolve(repoRoot, "apps/desktop/src-tauri/target/release");
  const candidates =
    platform() === "darwin"
      ? [resolve(base, "desktop"), resolve(base, "mantic-desktop"), resolve(base, "Mantic")]
      : platform() === "win32"
        ? [resolve(base, "desktop.exe"), resolve(base, "mantic-desktop.exe"), resolve(base, "Mantic.exe")]
        : [resolve(base, "desktop"), resolve(base, "mantic-desktop"), resolve(base, "mantic")];

  const found = candidates.find((p) => existsSync(p));
  if (!found) {
    throw new Error(
      `Could not find built binary. Run \`cd apps/desktop && pnpm build && cd src-tauri && cargo build --release\` first.\nChecked:\n${candidates.join("\n")}`,
    );
  }
  return found;
}

let mockServer: ChildProcess | undefined;

beforeAll(async () => {
  mockServer = spawn("pnpm", ["--filter", "mock-license", "dev"], {
    cwd: resolve(import.meta.dirname, ".."),
    stdio: ["ignore", "pipe", "pipe"],
    detached: true,
  });
  await sleep(3000);
});

afterAll(async () => {
  if (mockServer?.pid) {
    try {
      process.kill(-mockServer.pid, "SIGTERM");
    } catch {
      // already dead
    }
  }
});

describe("desktop binary smoke", () => {
  it("launches and stays alive for at least 5 seconds", async () => {
    const bin = findBinary();
    const proc = spawn(bin, [], {
      env: { ...process.env, MANTIC_LICENSE_SERVER: "http://localhost:4001" },
      stdio: ["ignore", "pipe", "pipe"],
    });

    let exitCode: number | null = null;
    let stderr = "";
    proc.on("exit", (code) => {
      exitCode = code;
    });
    proc.stderr?.on("data", (chunk) => {
      stderr += chunk.toString();
    });

    await sleep(5000);

    const stillAlive = exitCode === null;

    try {
      proc.kill("SIGTERM");
    } catch {
      // already dead
    }

    expect(stillAlive, `Binary exited early with code ${exitCode}. stderr:\n${stderr}`).toBe(true);
  });

  it("mock license server responds to healthz", async () => {
    const out = execSync("curl -sf http://localhost:4001/healthz", { encoding: "utf8" });
    const body = JSON.parse(out);
    expect(body.ok).toBe(true);
  });
});
