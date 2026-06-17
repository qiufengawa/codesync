import { create } from "zustand";

export type ThemeMode = "light" | "dark" | "system";
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "codesync:theme";

function readStoredMode(): ThemeMode {
  if (typeof window === "undefined") return "system";
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (raw === "light" || raw === "dark" || raw === "system") return raw;
  } catch {
    /* localStorage 不可用时回退到 system */
  }
  return "system";
}

function getSystemTheme(): ResolvedTheme {
  if (typeof window === "undefined" || !window.matchMedia) return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function resolveTheme(mode: ThemeMode): ResolvedTheme {
  return mode === "system" ? getSystemTheme() : mode;
}

function applyTheme(resolved: ResolvedTheme) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.classList.toggle("dark", resolved === "dark");
  root.style.colorScheme = resolved;
  root.dataset.theme = resolved;
}

let transitionTimer: number | null = null;
let transitionGeneration = 0;
function suspendThemeTransitions() {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  const generation = ++transitionGeneration;
  if (transitionTimer !== null) window.clearTimeout(transitionTimer);

  root.classList.add("theme-switching");
  void root.offsetHeight;

  window.requestAnimationFrame(() => {
    window.requestAnimationFrame(() => {
      if (generation !== transitionGeneration) return;
      root.classList.remove("theme-switching");
    });
  });

  transitionTimer = window.setTimeout(() => {
    if (generation !== transitionGeneration) return;
    root.classList.remove("theme-switching");
    transitionTimer = null;
  }, 120);
}

type State = {
  mode: ThemeMode;
  resolved: ResolvedTheme;
  setMode: (mode: ThemeMode) => void;
  cycle: () => void;
  toggle: () => void;
  init: () => () => void;
};

export const useTheme = create<State>((set, get) => ({
  mode: readStoredMode(),
  resolved: resolveTheme(readStoredMode()),

  setMode: (mode) => {
    try {
      window.localStorage.setItem(STORAGE_KEY, mode);
    } catch {
      /* ignore */
    }
    const resolved = resolveTheme(mode);
    const prevResolved = get().resolved;
    if (prevResolved !== resolved) suspendThemeTransitions();
    applyTheme(resolved);
    set({ mode, resolved });
  },

  cycle: () => {
    const cur = get().mode;
    const next: ThemeMode = cur === "light" ? "dark" : cur === "dark" ? "system" : "light";
    get().setMode(next);
  },

  toggle: () => {
    const cur = get().resolved;
    get().setMode(cur === "dark" ? "light" : "dark");
  },

  init: () => {
    applyTheme(get().resolved);

    if (typeof window === "undefined" || !window.matchMedia) {
      return () => {};
    }
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      if (get().mode !== "system") return;
      const resolved: ResolvedTheme = mq.matches ? "dark" : "light";
      if (get().resolved !== resolved) suspendThemeTransitions();
      applyTheme(resolved);
      set({ resolved });
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  },
}));

export function bootstrapTheme(): void {
  applyTheme(resolveTheme(readStoredMode()));
}
