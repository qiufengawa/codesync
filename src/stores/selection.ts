import { create } from "zustand";

type State = {
  selected: Set<string>;
  toggle: (id: string) => void;
  set: (ids: string[]) => void;
  clear: () => void;
  addMany: (ids: string[]) => void;
  removeMany: (ids: string[]) => void;
};

export const useSelection = create<State>((set) => ({
  selected: new Set<string>(),
  toggle: (id) =>
    set((s) => {
      const next = new Set(s.selected);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return { selected: next };
    }),
  set: (ids) => set({ selected: new Set(ids) }),
  clear: () => set({ selected: new Set() }),
  addMany: (ids) =>
    set((s) => {
      const next = new Set(s.selected);
      ids.forEach((i) => next.add(i));
      return { selected: next };
    }),
  removeMany: (ids) =>
    set((s) => {
      const next = new Set(s.selected);
      ids.forEach((i) => next.delete(i));
      return { selected: next };
    }),
}));
