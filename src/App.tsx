import { lazy, Suspense, useEffect } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { Toaster } from "sonner";

import { SidebarProvider } from "@/components/ui/sidebar";
import { Sidebar } from "@/components/Sidebar";
import { useHotkeys } from "@/hooks/useHotkeys";
import { webuiDefaultProvider } from "@/lib/runtime";
import { useSettings } from "@/stores/settings";
import { useTheme } from "@/stores/theme";

const SessionsRoute = lazy(() => import("@/routes/sessions"));
const BackupsRoute = lazy(() => import("@/routes/backups"));
const BackupDetailRoute = lazy(() => import("@/routes/backup-detail"));
const StatsRoute = lazy(() => import("@/routes/stats"));
const RepairRoute = lazy(() => import("@/routes/repair"));
const TransferRoute = lazy(() => import("@/routes/transfer"));

export default function App() {
  const load = useSettings((s) => s.load);
  const settings = useSettings((s) => s.settings);
  const initTheme = useTheme((s) => s.init);
  const toggleTheme = useTheme((s) => s.toggle);
  const defaultProvider = webuiDefaultProvider();
  const defaultSessionsPath = `/${defaultProvider}/sessions`;
  const defaultRepairPath = `/${defaultProvider}/repair`;
  const defaultBackupsPath = `/${defaultProvider}/backups`;
  const defaultTransferPath = `/${defaultProvider}/transfer`;

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    return initTheme();
  }, [initTheme]);

  useHotkeys([
    {
      combo: "mod+shift+l",
      handler: (e) => {
        e.preventDefault();
        toggleTheme();
      },
    },
  ]);

  return (
    <SidebarProvider
      tooltipDelayDuration={200}
      className="h-full min-h-0 overflow-hidden"
    >
      <Sidebar />
      <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
        <Suspense fallback={<RouteLoading />}>
          <Routes>
            <Route path="/" element={<Navigate to={defaultSessionsPath} replace />} />
            <Route path="/codex/sessions" element={<SessionsRoute provider="codex" />} />
            <Route path="/codex/repair" element={<RepairRoute provider="codex" />} />
            <Route path="/codex/backups" element={<BackupsRoute provider="codex" />} />
            <Route path="/codex/backups/:name" element={<BackupDetailRoute provider="codex" />} />
            <Route path="/codex/transfer" element={<TransferRoute provider="codex" />} />
            <Route path="/claude/sessions" element={<SessionsRoute provider="claude" />} />
            <Route path="/claude/repair" element={<RepairRoute provider="claude" />} />
            <Route path="/claude/backups" element={<BackupsRoute provider="claude" />} />
            <Route path="/claude/backups/:name" element={<BackupDetailRoute provider="claude" />} />
            <Route path="/claude/transfer" element={<TransferRoute provider="claude" />} />
            <Route path="/sessions" element={<Navigate to={defaultSessionsPath} replace />} />
            <Route path="/repair" element={<Navigate to={defaultRepairPath} replace />} />
            <Route path="/backups" element={<Navigate to={defaultBackupsPath} replace />} />
            <Route path="/backups/:name" element={<BackupDetailRoute provider={defaultProvider} />} />
            <Route path="/transfer" element={<Navigate to={defaultTransferPath} replace />} />
            <Route path="/stats" element={<StatsRoute />} />
            <Route path="*" element={<Navigate to="/codex/sessions" replace />} />
          </Routes>
        </Suspense>
      </main>
      <Toaster position="top-center" richColors closeButton />
      {!settings && <LoadingBoot />}
    </SidebarProvider>
  );
}

function RouteLoading() {
  return (
    <div className="flex h-full items-center justify-center p-6">
      <BootCard subtitle="正在加载页面" />
    </div>
  );
}

function LoadingBoot() {
  return (
    <div className="boot-splash pointer-events-none">
      <BootCard subtitle="正在加载设置" />
    </div>
  );
}

function BootCard({ subtitle }: { subtitle: string }) {
  return (
    <div className="boot-card">
      <div className="boot-brand">
        <div className="boot-mark">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M7 7.5 12 12l-5 4.5"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.4"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </div>
        <div className="boot-copy">
          <div className="boot-title">CC Sessions</div>
          <div className="boot-subtitle">{subtitle}</div>
        </div>
      </div>
      <div className="boot-track">
        <div className="boot-bar" />
      </div>
    </div>
  );
}
