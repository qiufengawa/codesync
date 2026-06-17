declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
    __CODESYNC_WEBUI__?: {
      apiToken?: string;
      defaultProvider?: "codex" | "claude" | "opencode";
    };
  }
}

export function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function isWebRuntime() {
  return typeof window !== "undefined" && !isTauriRuntime();
}

export function webuiApiToken() {
  return window.__CODESYNC_WEBUI__?.apiToken;
}

export function webuiDefaultProvider(): "codex" | "claude" | "opencode" {
  const provider = window.__CODESYNC_WEBUI__?.defaultProvider;
  return provider === "codex" || provider === "claude" ? provider : "opencode";
}
