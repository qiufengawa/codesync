import { type ReactNode } from "react";
import { cn } from "@/lib/utils";

type Props = {
  icon?: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
  className?: string;
};

export function EmptyState({ icon, title, description, action, className }: Props) {
  return (
    <div className={cn("flex flex-col items-center justify-center gap-4 py-24", className)}>
      {icon && (
        <div className="text-muted-foreground/25">{icon}</div>
      )}
      <div className="text-sm font-medium tracking-wide text-foreground">{title}</div>
      {description && <div className="max-w-sm text-center text-[13px] font-light text-muted-foreground">{description}</div>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}
