import { useState } from "react";
import { ChevronDown, FolderKanban } from "lucide-react";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Button } from "@/components/ui/button";
import { SessionCard } from "@/components/SessionCard";
import type { FamilyOverlay, SessionSummary } from "@/lib/api";
import { useSelection } from "@/stores/selection";
import { humanTokens, relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";

type Handlers = {
  onPreview: (s: SessionSummary) => void;
  onCopyResume: (s: SessionSummary) => void;
  onRevealCwd: (s: SessionSummary) => void;
  onArchiveToggle?: (s: SessionSummary) => void;
  onBackup?: (s: SessionSummary) => void;
  onDelete?: (s: SessionSummary) => void;
  onClone?: (s: SessionSummary) => void;
  onOpenFamily?: (s: SessionSummary) => void;
};

type Props = {
  cwd: string;
  cwdDisplay: string;
  sessions: SessionSummary[];
  query: string;
  handlers: Handlers;
  defaultOpen?: boolean;
  overlay?: Map<string, FamilyOverlay>;
  currentProvider?: string | null;
};

export function ProjectGroupView({
  cwd,
  cwdDisplay,
  sessions,
  query,
  handlers,
  defaultOpen = false,
  overlay,
  currentProvider,
}: Props) {
  const [open, setOpen] = useState(defaultOpen);
  const selected = useSelection((s) => s.selected);
  const toggle = useSelection((s) => s.toggle);
  const addMany = useSelection((s) => s.addMany);
  const removeMany = useSelection((s) => s.removeMany);

  const ids = sessions.map((s) => s.id);
  const allSelected = ids.every((id) => selected.has(id));
  const someSelected = !allSelected && ids.some((id) => selected.has(id));

  const latest = Math.max(...sessions.map((s) => s.updated_at));
  const tokens = sessions.reduce((a, b) => a + b.tokens_used, 0);

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="min-w-0 border-b border-border/30"
    >
      <CollapsibleTrigger asChild>
        <div
          className={cn(
            "flex min-w-0 cursor-pointer items-center gap-3 px-6 py-3 transition-colors",
            open ? "bg-muted/20" : "hover:bg-muted/20",
          )}
        >
          <ChevronDown
            className={cn(
              "h-3 w-3 shrink-0 text-muted-foreground/50 transition-transform",
              open && "rotate-180",
            )}
          />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <span className="min-w-0 truncate text-sm font-medium text-foreground">
                {cwdDisplay}
              </span>
              <span className="shrink-0 font-mono text-[11px] font-light tabular-nums text-muted-foreground/50">
                {sessions.length}
              </span>
              {tokens > 0 && (
                <span className="shrink-0 font-mono text-[11px] font-light tabular-nums text-muted-foreground/40">
                  {humanTokens(tokens)}
                </span>
              )}
            </div>
          </div>
          <span className="shrink-0 font-mono text-[11px] font-light tabular-nums text-muted-foreground/50">
            {relativeTime(latest)}
          </span>
          <Button
            variant="ghost"
            size="sm"
            onClick={(e) => {
              e.stopPropagation();
              if (allSelected) removeMany(ids);
              else addMany(ids);
            }}
            className="h-7 shrink-0 px-2 text-[11px] text-muted-foreground hover:text-foreground"
          >
            {allSelected ? "全不选" : someSelected ? "补全选" : "全选"}
          </Button>
        </div>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="min-w-0 border-t border-border/30">
          {sessions.map((s) => (
            <SessionCard
              key={s.id}
              s={s}
              selected={selected.has(s.id)}
              onToggleSelect={toggle}
              query={query}
              showProject={false}
              overlay={overlay?.get(s.id)}
              currentProvider={currentProvider}
              {...handlers}
            />
          ))}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
