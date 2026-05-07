import { useState, type ReactNode } from "react";
import { AlertTriangle, Loader2 } from "lucide-react";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

type Props = {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  title: string;
  confirmText: string;
  onConfirm: () => Promise<void> | void;
  children?: ReactNode;
  disableEnter?: boolean;
};

export function DangerDialog({
  open,
  onOpenChange,
  title,
  confirmText,
  onConfirm,
  children,
  disableEnter = true,
}: Props) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const onClick = async (e: React.MouseEvent) => {
    e.preventDefault();
    if (busy) return;
    setBusy(true);
    setErr(null);
    try {
      await onConfirm();
      onOpenChange(false);
    } catch (e: any) {
      setErr(String(e?.message ?? e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(v) => !busy && onOpenChange(v)}>
      <DialogContent
        onKeyDown={(e) => {
          if (disableEnter && e.key === "Enter") e.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle className="flex min-w-0 items-center gap-2 pr-6">
            <AlertTriangle className="h-5 w-5 shrink-0 text-destructive" />
            <span className="min-w-0 wrap-anywhere">{title}</span>
          </DialogTitle>
          <DialogDescription asChild>
            <div className="max-h-[52vh] min-w-0 max-w-full space-y-2 overflow-y-auto overflow-x-hidden pr-1 text-sm text-muted-foreground wrap-anywhere whitespace-normal">
              {children}
            </div>
          </DialogDescription>
        </DialogHeader>
        {err && (
          <div className="max-h-24 overflow-y-auto overflow-x-hidden rounded-md border border-destructive/40 bg-destructive/10 p-2 text-xs text-destructive wrap-anywhere">
            失败：{err}
          </div>
        )}
        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline" disabled={busy}>
              取消
            </Button>
          </DialogClose>
          <Button
            onClick={onClick}
            disabled={busy}
            className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
          >
            {busy && <Loader2 className="h-4 w-4 animate-spin" />}
            {confirmText}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
