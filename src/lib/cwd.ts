export function stripVerbatim(p: string): string {
  if (p.startsWith("\\\\?\\UNC\\")) return "\\\\" + p.slice("\\\\?\\UNC\\".length);
  if (p.startsWith("\\\\?\\")) return p.slice("\\\\?\\".length);
  return p;
}

export function basename(p: string): string {
  const s = stripVerbatim(p);
  const last = s.split(/[\\/]/).filter(Boolean).pop();
  return last || s;
}

export function dirname(p: string): string {
  const s = stripVerbatim(p).replace(/[\\/]+$/, "");
  const idx = Math.max(s.lastIndexOf("\\"), s.lastIndexOf("/"));
  if (idx <= 0) return s;
  return s.slice(0, idx);
}

export function joinPath(dir: string, child: string): string {
  const sep = dir.includes("\\") ? "\\" : "/";
  const base = dir.endsWith("\\") || dir.endsWith("/") ? dir.slice(0, -1) : dir;
  return `${base}${sep}${child}`;
}
