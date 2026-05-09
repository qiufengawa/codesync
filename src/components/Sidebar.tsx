import { NavLink, useLocation } from "react-router-dom";
import { Archive, BarChart3, MessageSquare, Package, Settings, Terminal, Wrench } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Sidebar as SidebarPrimitive,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import { SettingsSheet } from "@/components/SettingsSheet";
import { useSettings } from "@/stores/settings";

type NavItem = {
  to: string;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
};

const codexItems: NavItem[] = [
  { to: "/codex/sessions", icon: MessageSquare, label: "会话" },
  { to: "/codex/repair", icon: Wrench, label: "修复" },
  { to: "/codex/backups", icon: Archive, label: "备份" },
  { to: "/codex/transfer", icon: Package, label: "导出 / 导入" },
];

const claudeItems: NavItem[] = [
  { to: "/claude/sessions", icon: MessageSquare, label: "会话" },
  { to: "/claude/repair", icon: Wrench, label: "修复" },
  { to: "/claude/backups", icon: Archive, label: "备份" },
  { to: "/claude/transfer", icon: Package, label: "导出 / 导入" },
];

const globalItems: NavItem[] = [
  { to: "/stats", icon: BarChart3, label: "统计" },
];

export function Sidebar() {
  const settings = useSettings((s) => s.settings);
  const location = useLocation();

  return (
    <SidebarPrimitive
      collapsible="none"
      className="hidden w-56 shrink-0 border-r md:flex"
      style={{ "--sidebar-width": "14rem" } as React.CSSProperties}
    >
      <SidebarHeader className="flex h-14 flex-row items-center gap-2 border-b px-4 py-0">
        <div className="flex h-8 w-8 items-center justify-center rounded-md bg-foreground text-background">
          <Terminal className="h-4 w-4" />
        </div>
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold">CC Sessions</div>
          <div className="truncate text-[11px] text-muted-foreground">Codex / Claude</div>
        </div>
      </SidebarHeader>

      <SidebarContent>
        <NavGroup title="Codex" items={codexItems} pathname={location.pathname} />
        <NavGroup title="Claude" items={claudeItems} pathname={location.pathname} />
        <NavGroup title="全局" items={globalItems} pathname={location.pathname} />
      </SidebarContent>

      <SidebarFooter className="border-t p-2">
        {settings?.codex_dir && (
          <div className="rounded-md bg-muted/40 px-2.5 py-1.5 text-[11px] text-muted-foreground">
            <div className="mb-0.5 font-medium text-foreground/80">Codex 目录</div>
            <div className="truncate font-mono" title={settings.codex_dir}>
              {settings.codex_dir}
            </div>
          </div>
        )}
        {settings?.claude_dir && (
          <div className="rounded-md bg-muted/40 px-2.5 py-1.5 text-[11px] text-muted-foreground">
            <div className="mb-0.5 font-medium text-foreground/80">Claude 目录</div>
            <div className="truncate font-mono" title={settings.claude_dir}>
              {settings.claude_dir}
            </div>
          </div>
        )}
        <SettingsSheet
          trigger={
            <Button variant="ghost" size="sm" className="w-full justify-start gap-2">
              <Settings className="h-4 w-4" />
              设置
            </Button>
          }
        />
      </SidebarFooter>
    </SidebarPrimitive>
  );
}

function NavGroup({
  title,
  items,
  pathname,
}: {
  title: string;
  items: NavItem[];
  pathname: string;
}) {
  return (
    <SidebarGroup className="p-2 pb-1">
      <div className="px-3 pb-1.5 pt-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
        {title}
      </div>
      <SidebarMenu className="gap-0.5">
        {items.map((it) => {
          const isActive = it.to === "/stats" ? pathname === it.to : pathname.startsWith(it.to);
          return (
            <SidebarMenuItem key={it.to}>
              <SidebarMenuButton asChild isActive={isActive} className="h-auto px-3 py-2 text-sm font-medium">
                <NavLink to={it.to} end={false}>
                  <it.icon className="h-4 w-4" />
                  <span>{it.label}</span>
                </NavLink>
              </SidebarMenuButton>
            </SidebarMenuItem>
          );
        })}
      </SidebarMenu>
    </SidebarGroup>
  );
}
