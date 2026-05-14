import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { Monitor, Moon, Sun } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useTheme, type ThemeMode } from "@/stores/theme";
import { cn } from "@/lib/utils";

type Option = {
  value: ThemeMode;
  label: string;
  hint: string;
  Icon: React.ComponentType<{ className?: string }>;
};

const OPTIONS: Option[] = [
  { value: "light", label: "浅色", hint: "Light", Icon: Sun },
  { value: "system", label: "跟随系统", hint: "System", Icon: Monitor },
  { value: "dark", label: "深色", hint: "Dark", Icon: Moon },
];

export function ThemeToggle({ className }: { className?: string }) {
  const mode = useTheme((s) => s.mode);
  const setMode = useTheme((s) => s.setMode);
  const groupId = useId();
  const containerRef = useRef<HTMLDivElement>(null);
  const buttonRefs = useRef<Record<ThemeMode, HTMLButtonElement | null>>({
    light: null,
    system: null,
    dark: null,
  });
  const [indicator, setIndicator] = useState<{ x: number; w: number } | null>(null);

  useLayoutEffect(() => {
    const container = containerRef.current;
    const target = buttonRefs.current[mode];
    if (!container || !target) return;
    const cb = container.getBoundingClientRect();
    const tb = target.getBoundingClientRect();
    setIndicator({ x: tb.left - cb.left, w: tb.width });
  }, [mode]);

  useEffect(() => {
    const onResize = () => {
      const container = containerRef.current;
      const target = buttonRefs.current[mode];
      if (!container || !target) return;
      const cb = container.getBoundingClientRect();
      const tb = target.getBoundingClientRect();
      setIndicator({ x: tb.left - cb.left, w: tb.width });
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [mode]);

  return (
    <div
      role="radiogroup"
      aria-label="界面主题"
      ref={containerRef}
      className={cn(
        "relative isolate flex h-9 w-full items-center gap-0.5 rounded-lg border border-border/70 bg-muted/40 p-0.5 shadow-[inset_0_1px_0_0_hsl(var(--background)/0.6)] dark:bg-muted/30 dark:shadow-[inset_0_1px_0_0_hsl(var(--background)/0.2)]",
        className,
      )}
    >
      {indicator && (
        <span
          aria-hidden="true"
          className={cn(
            "pointer-events-none absolute top-1/2 -translate-y-1/2 rounded-md",
            "bg-background shadow-[0_1px_2px_-1px_hsl(var(--foreground)/0.18),0_0_0_1px_hsl(var(--border))]",
            "ring-1 ring-inset ring-foreground/[0.04]",
          )}
          style={{
            height: "calc(100% - 4px)",
            width: indicator.w,
            transform: `translate3d(${indicator.x}px, -50%, 0)`,
            transitionProperty: "transform, width",
            transitionDuration: "420ms",
            transitionTimingFunction: "cubic-bezier(0.32, 0.72, 0.16, 1.18)",
          }}
        />
      )}

      {OPTIONS.map((opt) => {
        const active = mode === opt.value;
        const Icon = opt.Icon;
        return (
          <Tooltip key={opt.value} delayDuration={300}>
            <TooltipTrigger asChild>
              <button
                ref={(el) => {
                  buttonRefs.current[opt.value] = el;
                }}
                type="button"
                role="radio"
                aria-checked={active}
                aria-label={opt.label}
                name={groupId}
                onClick={() => setMode(opt.value)}
                className={cn(
                  "group relative z-10 flex h-full flex-1 items-center justify-center rounded-md text-[12px] font-medium",
                  "outline-none focus-visible:ring-2 focus-visible:ring-ring/60 focus-visible:ring-offset-0",
                  "transition-colors duration-200",
                  active
                    ? "text-foreground"
                    : "text-muted-foreground/80 hover:text-foreground/90",
                )}
              >
                <span className="relative flex items-center justify-center">
                  <Icon
                    className={cn(
                      "h-[15px] w-[15px] transition-all duration-300 ease-out",
                      opt.value === "light" && active && "drop-shadow-[0_0_6px_hsl(45_100%_55%/0.45)]",
                      opt.value === "dark" && active && "drop-shadow-[0_0_6px_hsl(220_80%_70%/0.35)]",
                      active ? "scale-100" : "scale-[0.92] group-hover:scale-100",
                    )}
                    aria-hidden="true"
                  />
                  {opt.value === "light" && active && (
                    <span
                      aria-hidden="true"
                      className="pointer-events-none absolute inset-0 -m-1 animate-sun-rays opacity-50"
                    />
                  )}
                </span>
              </button>
            </TooltipTrigger>
            <TooltipContent side="top" sideOffset={6} className="px-2 py-1 text-[11px]">
              <span className="font-medium">{opt.label}</span>
              <span className="ml-1.5 font-mono text-[10px] text-muted-foreground">
                {opt.hint}
              </span>
            </TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );
}
