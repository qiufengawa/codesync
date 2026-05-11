import { useEffect } from "react";
import { shouldIgnoreGlobalHotkey } from "@/lib/keyboard";

type Handler = (e: KeyboardEvent) => void;

export function useHotkeys(bindings: Array<{ combo: string; handler: Handler }>) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (shouldIgnoreGlobalHotkey(e)) return;
      for (const b of bindings) {
        if (matches(e, b.combo)) {
          b.handler(e);
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [bindings]);
}

function matches(e: KeyboardEvent, combo: string): boolean {
  const parts = combo.toLowerCase().split("+");
  const key = parts[parts.length - 1];
  const needCtrl = parts.includes("ctrl") || parts.includes("mod") || parts.includes("cmd");
  const needShift = parts.includes("shift");
  const needAlt = parts.includes("alt");
  const ctrlOk = needCtrl ? e.ctrlKey || e.metaKey : !(e.ctrlKey || e.metaKey);
  const shiftOk = needShift ? e.shiftKey : !e.shiftKey;
  const altOk = needAlt ? e.altKey : !e.altKey;
  const keyOk =
    key === e.key.toLowerCase() ||
    key === e.code.toLowerCase().replace(/^key/, "") ||
    (key === "delete" && (e.key === "Delete" || e.key === "Backspace"));
  return ctrlOk && shiftOk && altOk && keyOk;
}
