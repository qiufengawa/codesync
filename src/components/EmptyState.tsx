import { type ReactNode } from "react";

type Props = {
  icon?: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
};

export function EmptyState({ icon, title, description, action }: Props) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 py-16 text-center text-muted-foreground">
      {icon && <div className="mb-1 text-muted-foreground/60">{icon}</div>}
      <div className="text-base font-medium text-foreground">{title}</div>
      {description && <div className="max-w-md text-sm">{description}</div>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}
