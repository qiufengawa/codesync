import { useMemo, useState } from "react";
import { ChevronDown } from "lucide-react";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import type { FamilyOverlay, SessionSummary } from "@/lib/api";
import { SessionCard } from "@/components/SessionCard";
import { ProjectGroupView } from "@/components/ProjectGroup";
import { bucketLabel, dayBucket } from "@/lib/format";
import { useSelection } from "@/stores/selection";
import { useView } from "@/stores/view";
import { cn } from "@/lib/utils";

type Handlers = {
  onPreview: (s: SessionSummary) => void;
  onCopyResume: (s: SessionSummary) => void;
  onRevealCwd: (s: SessionSummary) => void;
  onArchiveToggle?: (s: SessionSummary) => void;
  onBackup: (s: SessionSummary) => void;
  onDelete?: (s: SessionSummary) => void;
  onClone?: (s: SessionSummary) => void;
  onOpenFamily?: (s: SessionSummary) => void;
};

type Props = Handlers & {
  sessions: SessionSummary[];
  backupIndex?: Record<string, string[]>;
  overlay?: Map<string, FamilyOverlay>;
  currentProvider?: string | null;
};

type BucketKey = "today" | "yesterday" | "week" | "month" | "earlier";

export function SessionList({ sessions, backupIndex, overlay, currentProvider, ...h }: Props) {
  const view = useView((s) => s.view);
  const query = useView((s) => s.query);
  const prefillCwd = useView((s) => s.prefillCwd);

  const filtered = useMemo(() => {
    if (!prefillCwd) return sessions;
    return sessions.filter((s) => s.cwd === prefillCwd);
  }, [sessions, prefillCwd]);

  const enriched = useMemo(
    () =>
      filtered.map((s) => ({
        ...s,
        has_backup: backupIndex ? !!backupIndex[s.id]?.length : s.has_backup,
      })),
    [filtered, backupIndex],
  );

  return view === "time" ? (
    <TimeView
      sessions={enriched}
      handlers={h}
      query={query}
      overlay={overlay}
      currentProvider={currentProvider}
    />
  ) : (
    <ProjectView
      sessions={enriched}
      handlers={h}
      query={query}
      overlay={overlay}
      currentProvider={currentProvider}
    />
  );
}

function TimeView({
  sessions,
  handlers,
  query,
  overlay,
  currentProvider,
}: {
  sessions: SessionSummary[];
  handlers: Handlers;
  query: string;
  overlay?: Map<string, FamilyOverlay>;
  currentProvider?: string | null;
}) {
  const selected = useSelection((s) => s.selected);
  const toggle = useSelection((s) => s.toggle);
  const [collapsed, setCollapsed] = useState<Record<BucketKey, boolean>>({
    today: false,
    yesterday: false,
    week: false,
    month: false,
    earlier: true,
  });

  const groups = useMemo(() => {
    const map = new Map<BucketKey, SessionSummary[]>();
    for (const s of sessions) {
      const k = dayBucket(s.updated_at);
      if (!map.has(k)) map.set(k, []);
      map.get(k)!.push(s);
    }
    const order: BucketKey[] = ["today", "yesterday", "week", "month", "earlier"];
    return order.filter((k) => map.has(k)).map((k) => ({ key: k, items: map.get(k)! }));
  }, [sessions]);

  return (
    <div className="min-w-0 max-w-full space-y-4 overflow-hidden p-6">
      {groups.map((g) => (
        <Collapsible
          key={g.key}
          open={!collapsed[g.key]}
          onOpenChange={(open) =>
            setCollapsed((s) => ({ ...s, [g.key]: !open }))
          }
          className="min-w-0"
        >
          <CollapsibleTrigger asChild>
            <button className="group flex w-full items-center gap-2 rounded-md px-1 py-1 transition-colors hover:bg-muted/50">
              <ChevronDown
                className={cn(
                  "h-4 w-4 shrink-0 text-muted-foreground transition-transform",
                  collapsed[g.key] && "-rotate-90",
                )}
              />
              <h2 className="text-sm font-semibold text-foreground">
                {bucketLabel[g.key]}
              </h2>
              <span className="text-xs text-muted-foreground">· {g.items.length}</span>
              <div className="ml-2 h-px flex-1 bg-border" />
            </button>
          </CollapsibleTrigger>
          <CollapsibleContent className="data-[state=open]:animate-accordion-down data-[state=closed]:animate-accordion-up overflow-hidden">
            <div className="mt-3 min-w-0 space-y-3">
              {g.items.map((s) => (
                <SessionCard
                  key={s.id}
                  s={s}
                  selected={selected.has(s.id)}
                  onToggleSelect={toggle}
                  query={query}
                  overlay={overlay?.get(s.id)}
                  currentProvider={currentProvider}
                  {...handlers}
                />
              ))}
            </div>
          </CollapsibleContent>
        </Collapsible>
      ))}
    </div>
  );
}

function ProjectView({
  sessions,
  handlers,
  query,
  overlay,
  currentProvider,
}: {
  sessions: SessionSummary[];
  handlers: Handlers;
  query: string;
  overlay?: Map<string, FamilyOverlay>;
  currentProvider?: string | null;
}) {
  const groups = useMemo(() => {
    const map = new Map<string, SessionSummary[]>();
    for (const s of sessions) {
      if (!map.has(s.cwd)) map.set(s.cwd, []);
      map.get(s.cwd)!.push(s);
    }
    return Array.from(map.entries())
      .map(([cwd, items]) => ({
        cwd,
        cwd_display: items[0]?.cwd_display ?? cwd,
        items,
        latest: Math.max(...items.map((x) => x.updated_at)),
      }))
      .sort((a, b) => b.latest - a.latest);
  }, [sessions]);

  return (
    <div className="min-w-0 max-w-full space-y-3 overflow-hidden p-6">
      {groups.map((g) => (
        <ProjectGroupView
          key={g.cwd}
          cwd={g.cwd}
          cwdDisplay={g.cwd_display}
          sessions={g.items}
          query={query}
          handlers={handlers}
          overlay={overlay}
          currentProvider={currentProvider}
        />
      ))}
    </div>
  );
}
