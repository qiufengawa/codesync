declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
    __CC_SESSIONS_WEBUI__?: {
      apiToken?: string;
      defaultProvider?: "codex" | "claude";
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
  return window.__CC_SESSIONS_WEBUI__?.apiToken;
}

export function webuiDefaultProvider(): "codex" | "claude" {
  return window.__CC_SESSIONS_WEBUI__?.defaultProvider === "claude" ? "claude" : "codex";
}
