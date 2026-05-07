import { useCallback, useEffect, useState } from "react";
import { api, type BackupSummary, type SessionProvider } from "@/lib/api";
import { useSettings } from "@/stores/settings";

export function useBackups(provider?: SessionProvider) {
  const settings = useSettings((s) => s.settings);
  const [backups, setBackups] = useState<BackupSummary[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    if (!settings?.backup_dir) return;
    setLoading(true);
    try {
      const list = await api.listBackups(settings.backup_dir, provider);
      setBackups(list);
    } finally {
      setLoading(false);
    }
  }, [settings?.backup_dir, provider]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { backups, loading, refresh };
}

export function useBackupIndex(provider?: SessionProvider) {
  const { backups } = useBackups(provider);
  const [index, setIndex] = useState<Record<string, string[]>>({});

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const map: Record<string, string[]> = {};
      for (const b of backups) {
        try {
          const detail = await api.openBackup(b.path);
          for (const s of detail.manifest.sessions) {
            (map[s.id] ||= []).push(b.path);
          }
        } catch {
          // skip broken backup
        }
      }
      if (!cancelled) setIndex(map);
    })();
    return () => {
      cancelled = true;
    };
  }, [backups]);

  return index;
}
