import { type ReactNode } from "react";
import { useLocation } from "react-router-dom";
import { Archive, ArrowDown01, Clock, FolderKanban, RefreshCw, Trash2, X } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { SearchInput } from "@/components/SearchInput";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Separator } from "@/components/ui/separator";
import { useSelection } from "@/stores/selection";
import { useView, type View } from "@/stores/view";

type TopBarProps = {
  title?: string;
  stats?: string;
  onRefresh?: () => void;
  onBulkBackup?: () => void;
  onBulkDelete?: () => void;
  showListTools?: boolean;
  children?: ReactNode;
};

export type { TopBarProps };

export function TopBar({
  title,
  stats,
  onRefresh,
  onBulkBackup,
  onBulkDelete,
  showListTools,
  children,
}: TopBarProps) {
  const loc = useLocation();
  const view = useView((s) => s.view);
  const setView = useView((s) => s.setView);
  const query = useView((s) => s.query);
  const setQuery = useView((s) => s.setQuery);
  const selected = useSelection((s) => s.selected);
  const clearSel = useSelection((s) => s.clear);

  const isSessionsPath = loc.pathname.includes("/sessions");
  const isBackupsDetailPath = loc.pathname.match(/\/backups\/[^/]+$/);
  const autoShowTools = showListTools ?? (isSessionsPath || !!isBackupsDetailPath);
  const hasSelection = selected.size > 0;

  return (
    <header className="flex h-12 shrink-0 items-center gap-4 border-b border-border/50 px-6">
      {title && (
        <h1 className="shrink-0 text-sm font-medium tracking-wide text-foreground">
          {title}
        </h1>
      )}

      {autoShowTools && (
        <>
          {title && <Separator orientation="vertical" className="h-5" />}
          <SearchInput
            value={query}
            onChange={setQuery}
            className="min-w-0 flex-1 basis-52 sm:max-w-72"
          />
          <Tabs value={view} onValueChange={(v) => setView(v as View)} className="shrink-0">
            <TabsList className="h-7">
              <TabsTrigger value="time" className="gap-1 px-2 text-[11px] font-medium uppercase tracking-wider">
                <Clock className="h-3 w-3" />
                时间
              </TabsTrigger>
              <TabsTrigger value="project" className="gap-1 px-2 text-[11px] font-medium uppercase tracking-wider">
                <FolderKanban className="h-3 w-3" />
                项目
              </TabsTrigger>
              <TabsTrigger value="size" className="gap-1 px-2 text-[11px] font-medium uppercase tracking-wider">
                <ArrowDown01 className="h-3 w-3" />
                大小
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </>
      )}

      <div className="ml-auto flex shrink-0 items-center gap-2">
        {hasSelection && (
          <div className="flex h-7 items-center gap-1 px-1">
            <span className="flex items-center gap-1.5 font-mono text-[11px] font-medium text-primary">
              <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full bg-primary" />
              {selected.size}
            </span>
            <Separator orientation="vertical" className="mx-1 h-3.5 bg-border/50" />
            {onBulkBackup && (
              <Button
                variant="ghost"
                size="sm"
                onClick={onBulkBackup}
                className="h-7 gap-1.5 px-2 text-[12px] text-muted-foreground hover:text-foreground"
              >
                <Archive className="h-3 w-3" />
                备份
              </Button>
            )}
            {onBulkDelete && (
              <Button
                variant="ghost"
                size="sm"
                onClick={onBulkDelete}
                className="h-7 gap-1.5 px-2 text-[12px] text-destructive/80 hover:text-destructive"
              >
                <Trash2 className="h-3 w-3" />
                删除
              </Button>
            )}
            <Button
              variant="ghost"
              size="icon"
              onClick={clearSel}
              className="h-7 w-7 text-muted-foreground hover:text-foreground"
              aria-label="清除选择"
            >
              <X className="h-3 w-3" />
            </Button>
          </div>
        )}

        {children}

        {stats && (
          <span className="shrink-0 font-mono text-[11px] font-light tabular-nums text-muted-foreground/60">
            {stats}
          </span>
        )}

        {onRefresh && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                onClick={onRefresh}
                className="h-8 w-8 shrink-0 text-muted-foreground hover:text-foreground"
                aria-label="刷新"
              >
                <RefreshCw className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>刷新</TooltipContent>
          </Tooltip>
        )}
      </div>
    </header>
  );
}
