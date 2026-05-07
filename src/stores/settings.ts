import { create } from "zustand";
import { api, type Settings } from "@/lib/api";

type State = {
  settings: Settings | null;
  loading: boolean;
  load: () => Promise<void>;
  save: (patch: Partial<Settings>) => Promise<void>;
};

export const useSettings = create<State>((set, get) => ({
  settings: null,
  loading: false,
  async load() {
    set({ loading: true });
    try {
      const s = await api.getSettings();
      set({ settings: s });
    } finally {
      set({ loading: false });
    }
  },
  async save(patch) {
    const cur = get().settings;
    if (!cur) return;
    const next = { ...cur, ...patch };
    await api.saveSettings(next);
    set({ settings: next });
  },
}));
