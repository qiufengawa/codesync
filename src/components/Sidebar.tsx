import { useNavigate, useLocation } from "react-router-dom";
import {
  BarChart3,
  Archive,
  MessageSquare,
  Moon,
  Package,
  Settings as SettingsIcon,
  Sun,
  Terminal,
  Wrench,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { SettingsSheet } from "@/components/SettingsSheet";
import { ProviderIcon } from "@/components/ProviderIcon";
import { useActiveProvider } from "@/stores/provider";
import { useTheme } from "@/stores/theme";
import type { SessionProvider } from "@/lib/api";
import { cn } from "@/lib/utils";

type FuncKey = "sessions" | "repair" | "backups" | "transfer";

const FUNC_ITEMS: { key: FuncKey; icon: typeof MessageSquare; label: string }[] = [
  { key: "sessions", icon: MessageSquare, label: "会话" },
  { key: "repair", icon: Wrench, label: "修复" },
  { key: "backups", icon: Archive, label: "备份" },
  { key: "transfer", icon: Package, label: "迁移" },
];

const PROVIDERS: {
  key: SessionProvider;
  label: string;
  activeBg: string;
  activeText: string;
}[] = [
  { key: "opencode", label: "OpenCode", activeBg: "bg-sky-500/10", activeText: "text-sky-600 dark:text-sky-400" },
  { key: "codex", label: "Codex", activeBg: "bg-emerald-500/10", activeText: "text-emerald-600 dark:text-emerald-400" },
  { key: "claude", label: "Claude", activeBg: "bg-orange-500/10", activeText: "text-orange-600 dark:text-orange-400" },
];

export function Sidebar() {
  const navigate = useNavigate();
  const loc = useLocation();
  const activeProvider = useActiveProvider((s) => s.activeProvider);
  const setActiveProvider = useActiveProvider((s) => s.setActiveProvider);

  // Sync store from URL on mount and navigation
  const pathProvider = loc.pathname.split("/")[1];
  if (
    (pathProvider === "codex" || pathProvider === "claude" || pathProvider === "opencode") &&
    pathProvider !== activeProvider
  ) {
    setActiveProvider(pathProvider);
  }

  const currentFunc = loc.pathname.split("/")[2] || "sessions";
  const isStats = loc.pathname === "/stats";

  const goTo = (provider: SessionProvider, func: FuncKey) => {
    setActiveProvider(provider);
    navigate(`/${provider}/${func}`);
  };

  const activeMeta = PROVIDERS.find((p) => p.key === activeProvider) ?? PROVIDERS[0];

  return (
    <aside className="flex h-full w-12 shrink-0 flex-col items-center gap-0.5 border-r border-border/40 bg-sidebar py-4">
      {/* Logo */}
      <div className="mb-3 flex h-7 w-7 items-center justify-center bg-primary text-primary-foreground">
        <Terminal className="h-3.5 w-3.5" />
      </div>

      {/* Provider logos */}
      <div className="mb-1 flex flex-col items-center gap-1">
        {PROVIDERS.map((p) => {
          const isActive = activeProvider === p.key;
          return (
            <Tooltip key={p.key}>
              <TooltipTrigger asChild>
                <button
                  onClick={() => goTo(p.key, currentFunc as FuncKey)}
                  className={cn(
                    "flex h-7 w-7 items-center justify-center transition-all",
                    isActive ? p.activeBg : "opacity-50 hover:opacity-100 hover:bg-muted/60",
                  )}
                  aria-label={p.label}
                >
                  <ProviderIcon provider={p.key} className={cn("h-4 w-4 transition-transform", isActive && "scale-110")} />
                </button>
              </TooltipTrigger>
              <TooltipContent side="right">{p.label}</TooltipContent>
            </Tooltip>
          );
        })}
      </div>

      <div className="my-1 h-px w-5 bg-border/30" />

      {/* Function icons — colored by active provider */}
      <div className="flex flex-col items-center gap-1">
        {FUNC_ITEMS.map((item) => {
          const isActive = !isStats && currentFunc === item.key;
          const Icon = item.icon;
          return (
            <Tooltip key={item.key}>
              <TooltipTrigger asChild>
                <button
                  onClick={() => goTo(activeProvider, item.key)}
                  className={cn(
                    "relative flex h-7 w-7 items-center justify-center  transition-all",
                    isActive
                      ? cn(activeMeta.activeBg, activeMeta.activeText)
                      : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
                  )}
                  aria-label={item.label}
                >
                  {isActive && (
                    <span className="absolute -left-3 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-r-full bg-primary" />
                  )}
                  <Icon className="h-4 w-4" />
                </button>
              </TooltipTrigger>
              <TooltipContent side="right">
                {activeMeta.label} · {item.label}
              </TooltipContent>
            </Tooltip>
          );
        })}
      </div>

      <div className="my-1 h-px w-5 bg-border/30" />

      {/* Stats — global */}
      <div className="flex flex-col items-center gap-1">
        <Tooltip>
          <TooltipTrigger asChild>
            <button
              onClick={() => navigate("/stats")}
              className={cn(
                "flex h-7 w-7 items-center justify-center  transition-all",
                isStats
                  ? "bg-primary/10 text-primary"
                  : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
              )}
              aria-label="统计"
            >
              {isStats && (
                <span className="absolute -left-3 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-r-full bg-primary" />
              )}
              <BarChart3 className="h-4 w-4" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="right">统计</TooltipContent>
        </Tooltip>
      </div>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Bottom: theme + settings */}
      <div className="flex flex-col items-center gap-1">
        <ThemeIconToggle />
        <SettingsSheet
          trigger={
            <Button
              variant="ghost"
              size="icon"
              aria-label="设置"
              className="h-7 w-7  text-muted-foreground hover:text-foreground"
            >
              <SettingsIcon className="h-4 w-4" />
            </Button>
          }
        />
      </div>
    </aside>
  );
}

function ThemeIconToggle() {
  const mode = useTheme((s) => s.mode);
  const setMode = useTheme((s) => s.setMode);
  const next = mode === "dark" ? "light" : "dark";
  const Icon = mode === "dark" ? Sun : Moon;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          onClick={() => setMode(next)}
          className="flex h-7 w-7 items-center justify-center  text-muted-foreground transition-all hover:bg-muted/60 hover:text-foreground"
          aria-label="切换主题"
        >
          <Icon className="h-4 w-4" />
        </button>
      </TooltipTrigger>
      <TooltipContent side="right">
        {mode === "dark" ? "切换浅色" : "切换深色"}
      </TooltipContent>
    </Tooltip>
  );
}
