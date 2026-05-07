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
