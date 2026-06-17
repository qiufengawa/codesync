import type { FamilyOverlay, SessionSummary } from "@/lib/api";

export function isSubagentSession(
  session: SessionSummary,
  overlay?: FamilyOverlay,
): boolean {
  if (session.provider === "opencode" || session.source === "opencode") {
    return false;
  }
  return (
    overlay?.clone_state === "subagent" ||
    hasText(session.agent_nickname) ||
    hasText(session.agent_role) ||
    isSubagentSource(session.source)
  );
}

export function isSubagentSource(source: string | null | undefined): boolean {
  const normalized = source?.trim();
  if (!normalized) return false;
  if (normalized.toLowerCase() === "subagent") return true;
  try {
    const parsed = JSON.parse(normalized);
    return !!parsed && typeof parsed === "object" && "subagent" in parsed;
  } catch {
    return false;
  }
}

function hasText(value: string | null | undefined): boolean {
  return !!value?.trim();
}
