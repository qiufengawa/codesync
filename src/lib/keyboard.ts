export function shouldIgnoreGlobalHotkey(e: KeyboardEvent): boolean {
  if (e.defaultPrevented) return true;
  if (!(e.target instanceof Element)) return false;

  return !!e.target.closest(
    'input, textarea, select, [contenteditable]:not([contenteditable="false"]), [role="dialog"], [role="alertdialog"]',
  );
}
