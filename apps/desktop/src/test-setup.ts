import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Default mock for the Tauri bridge. Individual tests can override per-call.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));
