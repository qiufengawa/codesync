export function shouldIgnoreGlobalHotkey(e: KeyboardEvent): boolean {
  if (e.defaultPrevented) return true;

  return shouldIgnoreTextEditingHotkey(e.target) || isDialogHotkeyBoundary(e.target);
}

export function shouldIgnoreTextEditingHotkey(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;

  return !!target.closest(
    'input, textarea, select, [contenteditable]:not([contenteditable="false"]), [role="textbox"]',
  );
}

function isDialogHotkeyBoundary(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;

  return !!target.closest('[role="dialog"], [role="alertdialog"]');
}
