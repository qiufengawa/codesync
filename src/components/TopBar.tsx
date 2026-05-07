import { useLocation } from "react-router-dom";
import { Archive, Clock, FolderKanban, RefreshCw, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SearchInput } from "@/components/SearchInput";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Separator } from "@/components/ui/separator";
import { useSelection } from "@/stores/selection";
import { useView } from "@/stores/view";

type Props = {
  title?: string;
  /** 次级展示，如"12 条"，会放在右侧刷新按钮前（不再置于标题下方）。 */
  stats?: string;
  onRefresh?: () => void;
  onBulkBackup?: () => void;
  onBulkDelete?: () => void;
  showListTools?: boolean;
  children?: React.ReactNode;
};

export function TopBar({
  title,
  stats,
  onRefresh,
  onBulkBackup,
  onBulkDelete,
  showListTools,
  children,
}: Props) {
  const loc = useLocation();
  const view = useView((s) => s.view);
  const setView = useView((s) => s.setView);
  const query = useView((s) => s.query);
  const setQuery = useView((s) => s.setQuery);
  const selected = useSelection((s) => s.selected);
  const clearSel = useSelection((s) => s.clear);

  const autoShowTools =
    showListTools ??
    (loc.pathname.startsWith("/sessions") ||
      loc.pathname.startsWith("/codex/sessions") ||
      loc.pathname.startsWith("/claude/sessions") ||
      loc.pathname.startsWith("/backups/") ||
      loc.pathname.startsWith("/codex/backups/") ||
      loc.pathname.startsWith("/claude/backups/"));

  return (
    <header className="shrink-0 border-b bg-background">
      <div className="flex h-14 min-w-0 items-center gap-3 px-6">
        {title && (
          <h1 className="shrink-0 text-base font-semibold">{title}</h1>
        )}

        {autoShowTools && (
          <>
            {title && <Separator orientation="vertical" className="h-6" />}
            <SearchInput value={query} onChange={setQuery} className="w-80 min-w-0 max-w-full" />
            <Tabs value={view} onValueChange={(v) => setView(v as "time" | "project")}>
              <TabsList className="h-9">
                <TabsTrigger value="time" className="gap-1.5">
                  <Clock className="h-3.5 w-3.5" />
                  按时间
                </TabsTrigger>
                <TabsTrigger value="project" className="gap-1.5">
                  <FolderKanban className="h-3.5 w-3.5" />
                  按项目
                </TabsTrigger>
              </TabsList>
            </Tabs>
          </>
        )}

        <div className="ml-auto flex min-w-0 items-center gap-1.5">
          {selected.size > 0 && (
            <>
              <span className="text-xs text-muted-foreground">选中 {selected.size} 条</span>
              {onBulkBackup && (
                <Button variant="outline" size="sm" onClick={onBulkBackup} className="gap-1.5">
                  <Archive className="h-4 w-4" />
                  备份
                </Button>
              )}
              {onBulkDelete && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={onBulkDelete}
                  className="gap-1.5 border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
                >
                  <Trash2 className="h-4 w-4" />
                  删除
                </Button>
              )}
              <Button variant="ghost" size="icon" onClick={clearSel} className="h-8 w-8">
                <X className="h-4 w-4" />
              </Button>
              <Separator orientation="vertical" className="mx-0.5 h-6" />
            </>
          )}
          {children}
          {stats && (
            <span className="text-xs text-muted-foreground tabular-nums">{stats}</span>
          )}
          {onRefresh && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="ghost" size="icon" onClick={onRefresh} className="h-9 w-9">
                  <RefreshCw className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>刷新</TooltipContent>
            </Tooltip>
          )}
        </div>
      </div>
    </header>
  );
}
