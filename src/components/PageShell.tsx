import { type ReactNode } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { TopBar, type TopBarProps } from "@/components/TopBar";
import { cn } from "@/lib/utils";

type PageShellProps = {
  topBar?: Omit<TopBarProps, "children">;
  topBarChildren?: ReactNode;
  banners?: ReactNode;
  children: ReactNode;
  scrollable?: boolean;
};

export function PageShell({ topBar, topBarChildren, banners, children, scrollable = true }: PageShellProps) {
  return (
    <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
      {topBar && <TopBar {...topBar}>{topBarChildren}</TopBar>}
      {banners}
      {scrollable ? (
        <ScrollArea className="flex-1">{children}</ScrollArea>
      ) : (
        <div className="min-h-0 flex-1 overflow-hidden">{children}</div>
      )}
    </div>
  );
}

type PageContentProps = {
  children: ReactNode;
  className?: string;
};

export function PageContent({ children, className }: PageContentProps) {
  return (
    <div className={cn("mx-auto w-full max-w-6xl px-6 py-6", className)}>
      {children}
    </div>
  );
}
