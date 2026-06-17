import { create } from "zustand";
import type { SessionProvider } from "@/lib/api";

type State = {
  activeProvider: SessionProvider;
  setActiveProvider: (p: SessionProvider) => void;
};

export const useActiveProvider = create<State>((set) => ({
  activeProvider: "opencode",
  setActiveProvider: (p) => set({ activeProvider: p }),
}));
