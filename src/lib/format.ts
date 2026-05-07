import { formatDistanceToNow, format, parseISO, isValid } from "date-fns";
import { zhCN } from "date-fns/locale";

export function humanBytes(n: number): string {
  if (!n || n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}

export function humanTokens(n: number): string {
  if (!n) return "0";
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}K`;
  if (n < 1_000_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  return `${(n / 1_000_000_000).toFixed(1)}B`;
}

/** 带单位的 token 展示，例如 "5.1M token"。 */
export function tokenLabel(n: number): string {
  return `${humanTokens(n)} token`;
}

export function shortId(id: string, len = 8): string {
  return id.slice(0, len);
}

export function fromUnix(ts: number): Date {
  return new Date(ts * 1000);
}

export function relativeTime(ts: number): string {
  if (!ts) return "—";
  try {
    return formatDistanceToNow(fromUnix(ts), { addSuffix: true, locale: zhCN });
  } catch {
    return "—";
  }
}

export function absoluteTime(ts: number): string {
  if (!ts) return "—";
  return format(fromUnix(ts), "yyyy-MM-dd HH:mm:ss");
}

/**
 * 将任意时间字符串（ISO 8601 / RFC3339 等）格式化为 "YYYY-MM-DD HH:mm:ss"。
 * - 若输入已是目标格式则直接返回。
 * - 若解析失败则原样返回。
 */
export function formatTimeString(s: string | null | undefined): string {
  if (!s) return "";
  if (/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/.test(s)) return s;
  try {
    const d = /^\d{4}-\d{2}-\d{2}T/.test(s) ? parseISO(s) : new Date(s);
    if (!isValid(d)) return s;
    return format(d, "yyyy-MM-dd HH:mm:ss");
  } catch {
    return s;
  }
}

export function dayBucket(ts: number): "today" | "yesterday" | "week" | "month" | "earlier" {
  const now = new Date();
  const d = fromUnix(ts);
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const startOfYesterday = startOfToday - 86400 * 1000;
  const startOfWeek = startOfToday - 6 * 86400 * 1000;
  const startOfMonth = startOfToday - 29 * 86400 * 1000;
  const t = d.getTime();
  if (t >= startOfToday) return "today";
  if (t >= startOfYesterday) return "yesterday";
  if (t >= startOfWeek) return "week";
  if (t >= startOfMonth) return "month";
  return "earlier";
}

export const bucketLabel: Record<ReturnType<typeof dayBucket>, string> = {
  today: "今天",
  yesterday: "昨天",
  week: "本周",
  month: "本月",
  earlier: "更早",
};

export function highlight(text: string, query: string): Array<{ t: string; hit: boolean }> {
  if (!query) return [{ t: text, hit: false }];
  const q = query.toLowerCase();
  const low = text.toLowerCase();
  const out: Array<{ t: string; hit: boolean }> = [];
  let i = 0;
  while (i < text.length) {
    const pos = low.indexOf(q, i);
    if (pos === -1) {
      out.push({ t: text.slice(i), hit: false });
      break;
    }
    if (pos > i) out.push({ t: text.slice(i, pos), hit: false });
    out.push({ t: text.slice(pos, pos + q.length), hit: true });
    i = pos + q.length;
  }
  return out;
}
