import { memo } from "react";
import {
  Archive,
  CheckCircle2,
  Copy,
  Eye,
  FolderOpen,
  GitBranch,
  Inbox,
  MoreHorizontal,
  Network,
  RotateCw,
  ShieldCheck,
  Trash2,
  Undo2,
} from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { FamilyOverlay, SessionSummary } from "@/lib/api";
import {
  absoluteTime,
  highlight,
  humanBytes,
  humanTokens,
  relativeTime,
  shortId,
} from "@/lib/format";
import { isSubagentSession } from "@/lib/sessionSource";
import { sessionDisplayPreview, sessionDisplayTitle } from "@/lib/sessionText";
import { cn } from "@/lib/utils";

type Props = {
  s: SessionSummary;
  selected: boolean;
  onToggleSelect: (id: string) => void;
  onPreview: (s: SessionSummary) => void;
  onCopyResume: (s: SessionSummary) => void;
  onRevealCwd: (s: SessionSummary) => void;
  onArchiveToggle?: (s: SessionSummary) => void;
  onBackup?: (s: SessionSummary) => void;
  onDelete?: (s: SessionSummary) => void;
  onClone?: (s: SessionSummary) => void;
  onOpenFamily?: (s: SessionSummary) => void;
  query?: string;
  showProject?: boolean;
  overlay?: FamilyOverlay;
  currentProvider?: string | null;
};

export const SessionCard = memo(function SessionCard({
  s,
  selected,
  onToggleSelect,
  onPreview,
  onCopyResume,
  onRevealCwd,
  onArchiveToggle,
  onBackup,
  onDelete,
  onClone,
  onOpenFamily,
  query = "",
  showProject = true,
  overlay,
  currentProvider,
}: Props) {
  const displayTitle = sessionDisplayTitle(s.title, s.first_user_message);
  const displayFirstUserMessage = sessionDisplayPreview(s.first_user_message);
  const syncAction = syncActionLabel(overlay?.clone_state, currentProvider);
  const isSubagent = isSubagentSession(s, overlay);
  const subagent = subagentLabel(s, isSubagent);
  const canCopyResume = !(s.provider === "claude" && isSubagent);

  return (
    <div
      className={cn(
        "group relative flex w-full items-start gap-4 px-4 py-4 transition-colors",
        "border-b border-border/40 last:border-b-0",
        selected && "bg-primary/[0.025]",
        !selected && "hover:bg-muted/30",
        s.archived && "opacity-50",
      )}
    >
      <Checkbox
        checked={selected}
        onCheckedChange={() => onToggleSelect(s.id)}
        className="mt-1 shrink-0"
        aria-label="选择会话"
      />

      <div className="min-w-0 flex-1 cursor-pointer" onClick={() => onPreview(s)}>
        {/* Title line */}
        <div className="flex min-w-0 items-center gap-2">
          <span className="min-w-0 truncate text-sm font-medium text-foreground">
            <Hl text={displayTitle} q={query} />
          </span>
          {s.archived && (
            <span className="shrink-0 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
              Archived
            </span>
          )}
          {s.has_backup && (
            <ShieldCheck className="h-3 w-3 shrink-0 text-emerald-500/70" />
          )}
          {subagent && (
            <span className="shrink-0 text-[10px] font-medium uppercase tracking-wider text-violet-500/80">
              {subagent.label}
            </span>
          )}
          {syncAction && (
            <span
              className="shrink-0 cursor-pointer text-[10px] font-medium uppercase tracking-wider text-blue-500/80 hover:text-blue-600"
              onClick={(e) => {
                e.stopPropagation();
                onClone?.(s);
              }}
            >
              {syncAction}
            </span>
          )}
          {overlay?.clone_state === "has_clone" && (
            <CheckCircle2 className="h-3 w-3 shrink-0 text-emerald-500/70" />
          )}
        </div>

        {/* Preview text */}
        {displayFirstUserMessage && (
          <p className="mt-1 line-clamp-1 min-w-0 text-[13px] font-light text-muted-foreground">
            <Hl text={displayFirstUserMessage} q={query} />
          </p>
        )}

        {/* Meta line */}
        <div className="mt-1.5 flex min-w-0 flex-wrap items-center gap-x-2.5 gap-y-0.5 font-mono text-[11px] text-muted-foreground/70">
          {showProject && s.cwd_display && (
            <>
              <span className="min-w-0 truncate">{s.cwd_display}</span>
              <MetaDot />
            </>
          )}
          <span>{shortId(s.id)}</span>
          {s.model && (
            <>
              <MetaDot />
              <span className="max-w-32 truncate">{s.model}</span>
            </>
          )}
          <MetaDot />
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="cursor-default whitespace-nowrap">{relativeTime(s.updated_at)}</span>
            </TooltipTrigger>
            <TooltipContent>{absoluteTime(s.updated_at)}</TooltipContent>
          </Tooltip>
        </div>
      </div>

      {/* Right: stats + actions */}
      <div className="flex shrink-0 items-center gap-3">
        {s.tokens_used > 0 && (
          <div className="hidden flex-col items-end sm:flex">
            <span className="font-mono text-[11px] font-medium tabular-nums text-foreground/60">
              {humanTokens(s.tokens_used)}
            </span>
            <span className="font-mono text-[10px] tabular-nums text-muted-foreground/50">
              {humanBytes(s.rollout_bytes)}
            </span>
          </div>
        )}

        <div className="flex items-center gap-0.5">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => onPreview(s)}
            className="h-8 w-8 text-muted-foreground hover:text-foreground"
          >
            <Eye className="h-3.5 w-3.5" />
          </Button>
          {canCopyResume && (
            <Button
              variant="ghost"
              size="icon"
              onClick={() => onCopyResume(s)}
              className="h-8 w-8 text-muted-foreground hover:text-foreground"
            >
              <Copy className="h-3.5 w-3.5" />
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon"
            onClick={() => onRevealCwd(s)}
            className="h-8 w-8 text-muted-foreground hover:text-foreground"
          >
            <FolderOpen className="h-3.5 w-3.5" />
          </Button>

          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8 text-muted-foreground hover:text-foreground"
              >
                <MoreHorizontal className="h-4 w-4" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              {onBackup && (
                <DropdownMenuItem onClick={() => onBackup(s)}>
                  <Archive className="h-4 w-4" />
                  单条备份
                </DropdownMenuItem>
              )}
              {onOpenFamily && (
                <DropdownMenuItem onClick={() => onOpenFamily(s)}>
                  <Network className="h-4 w-4" />
                  查看分支
                </DropdownMenuItem>
              )}
              {onClone && syncAction && (
                <DropdownMenuItem onClick={() => onClone(s)}>
                  <RotateCw className="h-4 w-4" />
                  {syncAction}
                </DropdownMenuItem>
              )}
              {onArchiveToggle && (
                <DropdownMenuItem onClick={() => onArchiveToggle(s)}>
                  {s.archived ? <Undo2 className="h-4 w-4" /> : <Inbox className="h-4 w-4" />}
                  {s.archived ? "取消归档" : "归档"}
                </DropdownMenuItem>
              )}
              {onDelete && (
                <>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem
                    onClick={() => onDelete(s)}
                    className="text-destructive focus:text-destructive"
                  >
                    <Trash2 className="h-4 w-4" />
                    删除会话
                  </DropdownMenuItem>
                </>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
    </div>
  );
});

function MetaDot() {
  return <span aria-hidden="true" className="text-muted-foreground/30">·</span>;
}

function Hl({ text, q }: { text: string; q: string }) {
  const parts = highlight(text, q);
  return (
    <>
      {parts.map((p, i) => (p.hit ? <mark key={i}>{p.t}</mark> : <span key={i}>{p.t}</span>))}
    </>
  );
}

function syncActionLabel(cloneState: string | undefined, currentProvider: string | null | undefined): string {
  if (cloneState === "resync") return "修复本地索引";
  if (cloneState === "clonable" && currentProvider) return `同步到 ${currentProvider}`;
  return "";
}

function subagentLabel(
  s: SessionSummary,
  isSubagent: boolean,
): { label: string; title: string } | null {
  if (!isSubagent) return null;
  const role = s.agent_role?.trim();
  const nickname = s.agent_nickname?.trim();
  const providerLabel = s.provider === "claude" ? "Claude" : "Codex";
  return {
    label: role ? `子代理 · ${role}` : "子代理",
    title: nickname
      ? `${providerLabel} 子代理线程：${nickname}${role ? `（${role}）` : ""}`
      : role
        ? `${providerLabel} 子代理线程：${role}`
        : `${providerLabel} 子代理线程`,
  };
}
