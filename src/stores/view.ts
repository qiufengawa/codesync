import { create } from "zustand";

export type View = "time" | "project" | "size";

type State = {
  view: View;
  query: string;
  showSubagentSessions: boolean;
  setView: (v: View) => void;
  setQuery: (q: string) => void;
  setShowSubagentSessions: (v: boolean) => void;
  prefillCwd: string | null;
  setPrefillCwd: (cwd: string | null) => void;
};

export const useView = create<State>((set) => ({
  view: "time",
  query: "",
  showSubagentSessions: false,
  prefillCwd: null,
  setView: (v) => set({ view: v }),
  setQuery: (q) => set({ query: q }),
  setShowSubagentSessions: (v) => set({ showSubagentSessions: v }),
  setPrefillCwd: (cwd) => set({ prefillCwd: cwd }),
}));
