import { create } from "zustand";

export type View = "time" | "project" | "size";

type State = {
  view: View;
  query: string;
  setView: (v: View) => void;
  setQuery: (q: string) => void;
  prefillCwd: string | null;
  setPrefillCwd: (cwd: string | null) => void;
};

export const useView = create<State>((set) => ({
  view: "time",
  query: "",
  prefillCwd: null,
  setView: (v) => set({ view: v }),
  setQuery: (q) => set({ query: q }),
  setPrefillCwd: (cwd) => set({ prefillCwd: cwd }),
}));
