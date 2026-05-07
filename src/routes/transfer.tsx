import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Archive,
  CheckCircle2,
  Copy,
  Download,
  FileArchive,
  FolderOpen,
  Loader2,
  Package,
  ShieldAlert,
  Upload,
} from "lucide-react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";

import { TopBar } from "@/components/TopBar";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { Checkbox } from "@/components/ui/checkbox";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  RadioGroup,
  RadioGroupItem,
} from "@/components/ui/radio-group";

import { useSettings } from "@/stores/settings";
import { useSessions } from "@/hooks/useSessions";
import { api, type BundleListItem, type ImportMode, type SessionProvider } from "@/lib/api";
import { humanBytes } from "@/lib/format";

export default function TransferRoute({ provider = "codex" }: { provider?: SessionProvider }) {
  const settings = useSettings((s) => s.settings);
  const codexDir = settings?.codex_dir ?? "";
  const claudeDir = settings?.claude_dir ?? "";
  const providerLabel = provider === "codex" ? "Codex" : "Claude";
  const providerDir = provider === "codex" ? codexDir : claudeDir;

  return (
    <>
      <TopBar title={`${providerLabel} 导出 / 导入`} showListTools={false} />
      <ScrollArea className="flex-1">
        <div className="p-6">
          {!providerDir ? (
            <EmptyState
              icon={<Package className="h-10 w-10" />}
              title={`尚未配置 ${providerLabel} 目录`}
            />
          ) : (
            <Tabs defaultValue="export" className="space-y-4">
              <TabsList>
                <TabsTrigger value="export" className="gap-1.5">
                  <Upload className="h-3.5 w-3.5" /> 导出会话数据
                </TabsTrigger>
                <TabsTrigger value="import" className="gap-1.5">
                  <Download className="h-3.5 w-3.5" /> 导入会话数据
                </TabsTrigger>
              </TabsList>
              <TabsContent value="export">
                <ExportPanel provider={provider} codexDir={codexDir} claudeDir={claudeDir} />
              </TabsContent>
              <TabsContent value="import">
                <ImportPanel provider={provider} codexDir={codexDir} claudeDir={claudeDir} />
              </TabsContent>
            </Tabs>
          )}
        </div>
      </ScrollArea>
    </>
  );
}

// ========================= 导出 =========================

function ExportPanel({
  provider,
  codexDir,
  claudeDir,
}: {
  provider: SessionProvider;
  codexDir: string;
  claudeDir: string;
}) {
  const { sessions, loading: loadingSessions } = useSessions(provider, "");
  const [outDir, setOutDir] = useState("");
  const [machineLabel, setMachineLabel] = useState("");
  const [exportGroup, setExportGroup] = useState("default");
  const [activeOnly, setActiveOnly] = useState(true);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [running, setRunning] = useState(false);
  const [lastOutRoot, setLastOutRoot] = useState<string | null>(null);

  const pickDir = async () => {
    const picked = await openDialog({ directory: true, defaultPath: outDir || undefined });
    if (typeof picked === "string") setOutDir(picked);
  };

  const toggle = (id: string) => {
    setSelectedIds((prev) => {
      const n = new Set(prev);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  };

  const visibleSessions = useMemo(() => sessions.slice(0, 500), [sessions]);
  const allSelected =
    visibleSessions.length > 0 &&
    visibleSessions.every((s) => selectedIds.has(s.id));
  const someSelected =
    !allSelected && visibleSessions.some((s) => selectedIds.has(s.id));
  const toggleAll = () => {
    setSelectedIds((prev) => {
      if (allSelected) {
        const n = new Set(prev);
        for (const s of visibleSessions) n.delete(s.id);
        return n;
      }
      const n = new Set(prev);
      for (const s of visibleSessions) n.add(s.id);
      return n;
    });
  };
  const copyId = async (id: string) => {
    try {
      await navigator.clipboard.writeText(id);
      toast.success("已复制到剪贴板");
    } catch {
      toast.error("复制失败");
    }
  };

  const exportSelected = async () => {
    if (!outDir) return toast.error("请先选择导出目录");
    if (selectedIds.size === 0) return toast.error("请先勾选要导出的会话");
    setRunning(true);
    try {
      const r = await api.exportSessionBundles({
        provider,
        codex_dir: codexDir,
        claude_dir: claudeDir,
        out_dir: outDir,
        ids: Array.from(selectedIds),
        machine_label: machineLabel || undefined,
        export_group: exportGroup || undefined,
      });
      const ok = r.filter((x) => x.ok).length;
      toast.success(`已导出 ${ok}/${r.length} 条会话数据`);
      setLastOutRoot(outDir);
    } catch (e) {
      toast.error(String((e as Error)?.message ?? e));
    } finally {
      setRunning(false);
    }
  };

  const exportAll = async () => {
    if (!outDir) return toast.error("请先选择导出目录");
    setRunning(true);
    try {
      const r = await api.exportAllBundles({
        provider,
        codex_dir: codexDir,
        claude_dir: claudeDir,
        out_dir: outDir,
        machine_label: machineLabel || undefined,
        export_group: exportGroup || undefined,
        active_only: activeOnly,
      });
      const ok = r.filter((x) => x.ok).length;
      toast.success(`已导出 ${ok}/${r.length} 条会话数据${activeOnly ? "（仅包含活跃记录）" : ""}`);
      setLastOutRoot(outDir);
    } catch (e) {
      toast.error(String((e as Error)?.message ?? e));
    } finally {
      setRunning(false);
    }
  };

  const packZip = async () => {
    if (!lastOutRoot && !outDir) return toast.error("请先导出到某个目录");
    const src = lastOutRoot ?? outDir;
    const zipPath = await saveDialog({
      defaultPath: `${provider}-bundles-${Date.now()}.zip`,
      filters: [{ name: "Zip", extensions: ["zip"] }],
    });
    if (!zipPath) return;
    setRunning(true);
    try {
      const r = await api.packBundlesZip(src, zipPath);
      toast.success(`打包完成：${r.files} 个文件 · ${humanBytes(r.bytes)}`);
    } catch (e) {
      toast.error(String((e as Error)?.message ?? e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="flex items-center gap-2 text-base">
            <FolderOpen className="h-4 w-4" />
            导出目标
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <div className="space-y-1.5">
              <Label className="text-xs">导出目录</Label>
              <div className="flex gap-1.5">
                <Input
                  value={outDir}
                  onChange={(e) => setOutDir(e.target.value)}
                  placeholder="选一个本地目录"
                  className="font-mono text-xs"
                />
                <Button
                  variant="outline"
                  size="sm"
                  onClick={pickDir}
                  className="shrink-0"
                >
                  浏览
                </Button>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label className="text-xs">机器标签</Label>
                <Input
                  value={machineLabel}
                  onChange={(e) => setMachineLabel(e.target.value)}
                  placeholder="默认取主机名"
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs">export group</Label>
                <Input
                  value={exportGroup}
                  onChange={(e) => setExportGroup(e.target.value)}
                  placeholder="default"
                />
              </div>
            </div>
          </div>
          <Separator />
          <div className="flex flex-wrap items-center gap-3">
            {provider === "codex" && (
              <div className="flex items-center gap-2">
                <Switch
                  id="active-only"
                  checked={activeOnly}
                  onCheckedChange={setActiveOnly}
                />
                <Label htmlFor="active-only" className="text-xs">
                  仅导出当前分支（分支记录）
                </Label>
              </div>
            )}
            <div className="ml-auto flex gap-1.5">
              <Button
                size="sm"
                variant="outline"
                onClick={exportSelected}
                disabled={running || selectedIds.size === 0}
                className="gap-1.5"
              >
                <Upload className="h-3.5 w-3.5" />
                导出勾选（{selectedIds.size}）
              </Button>
              <Button
                size="sm"
                onClick={exportAll}
                disabled={running}
                className="gap-1.5"
              >
                <Upload className="h-3.5 w-3.5" />
                导出全部
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={packZip}
                disabled={running}
                className="gap-1.5"
              >
                <FileArchive className="h-3.5 w-3.5" />
                打包成 zip
              </Button>
            </div>
          </div>
          {running && (
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" /> 执行中…
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="flex items-center gap-2 text-base">
            <Archive className="h-4 w-4" />
            会话（勾选即可逐条导出）
            <Badge variant="secondary" className="h-5 px-1.5 font-normal">
              {sessions.length}
            </Badge>
            {visibleSessions.length > 0 && (
              <span className="ml-auto text-xs font-normal text-muted-foreground">
                已选 {visibleSessions.filter((s) => selectedIds.has(s.id)).length}/
                {visibleSessions.length}
              </span>
            )}
          </CardTitle>
        </CardHeader>
        <CardContent>
          {loadingSessions ? (
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" /> 加载中…
            </div>
          ) : sessions.length === 0 ? (
            <EmptyState title="没有会话可导出" />
          ) : (
            <div className="rounded-md border">
              <div className="grid grid-cols-[2rem_8rem_minmax(0,1fr)_9rem_5rem] items-center gap-2 border-b bg-muted/40 px-3 py-2 text-[11px] font-medium text-muted-foreground">
                <Checkbox
                  checked={allSelected ? true : someSelected ? "indeterminate" : false}
                  onCheckedChange={toggleAll}
                  aria-label="全选当前列表"
                />
                <span>id</span>
                <span>标题</span>
                <span>model</span>
                <span className="text-right">tokens</span>
              </div>
              <ScrollArea className="h-96">
                <ul className="divide-y text-xs">
                  {visibleSessions.map((s) => (
                    <li
                      key={s.id}
                      className={`grid grid-cols-[2rem_8rem_minmax(0,1fr)_9rem_5rem] items-center gap-2 px-3 py-2 ${
                        selectedIds.has(s.id)
                          ? "bg-primary/5"
                          : "hover:bg-muted/30"
                      }`}
                    >
                      <Checkbox
                        checked={selectedIds.has(s.id)}
                        onCheckedChange={() => toggle(s.id)}
                        aria-label="选择该会话"
                      />
                      <Tooltip>
                        <TooltipTrigger asChild>
                          <button
                            type="button"
                            onClick={() => copyId(s.id)}
                            className="flex min-w-0 items-center gap-1 truncate text-left font-mono text-[11px] text-muted-foreground hover:text-foreground"
                            aria-label="复制会话 ID"
                          >
                            <span className="truncate">{s.id.slice(0, 8)}…</span>
                            <Copy className="h-3 w-3 shrink-0 opacity-60" />
                          </button>
                        </TooltipTrigger>
                        <TooltipContent className="font-mono text-[11px]">
                          {s.id} · 点击复制
                        </TooltipContent>
                      </Tooltip>
                      <span className="truncate" title={s.title || s.first_user_message || ""}>
                        {s.title || s.first_user_message || "—"}
                      </span>
                      <span className="truncate text-muted-foreground" title={s.model ?? ""}>
                        {s.model ?? "—"}
                      </span>
                      <span className="text-right tabular-nums text-muted-foreground">
                        {s.tokens_used}
                      </span>
                    </li>
                  ))}
                </ul>
              </ScrollArea>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

// ========================= 导入 =========================

function ImportPanel({
  provider,
  codexDir,
  claudeDir,
}: {
  provider: SessionProvider;
  codexDir: string;
  claudeDir: string;
}) {
  const [srcDir, setSrcDir] = useState("");
  const [items, setItems] = useState<BundleListItem[]>([]);
  const [mode, setMode] = useState<ImportMode>("skip");
  const [makeVisible, setMakeVisible] = useState(true);
  const [strict, setStrict] = useState(true);
  const [running, setRunning] = useState(false);
  const [scanLoading, setScanLoading] = useState(false);

  const pickDir = async () => {
    const picked = await openDialog({ directory: true });
    if (typeof picked === "string") {
      setSrcDir(picked);
    }
  };

  const pickZip = async () => {
    const picked = await openDialog({
      filters: [{ name: "Zip", extensions: ["zip"] }],
    });
    if (typeof picked === "string") {
      const tmp = await openDialog({
        directory: true,
        title: "选择解压目标目录",
      });
      if (typeof tmp === "string") {
        setRunning(true);
        try {
          const r = await api.unpackZip(picked, tmp);
          toast.success(`解压完成：${r.files} 文件 · ${humanBytes(r.bytes)}`);
          setSrcDir(tmp);
          await rescan(tmp);
        } catch (e) {
          toast.error(String((e as Error)?.message ?? e));
        } finally {
          setRunning(false);
        }
      }
    }
  };

  const rescan = useCallback(
    async (dir: string) => {
      if (!dir) return;
      setScanLoading(true);
      try {
        const r = await api.verifyBundlesCmd(dir, provider);
        setItems(r);
      } catch (e) {
        toast.error(String((e as Error)?.message ?? e));
      } finally {
        setScanLoading(false);
      }
    },
    [],
  );

  useEffect(() => {
    if (srcDir) void rescan(srcDir);
  }, [srcDir, rescan]);

  const runImport = async () => {
    if (!srcDir) return toast.error("请先选择数据所在的目录或解压好的 zip 文件夹");
    setRunning(true);
    try {
      const r = await api.importSessionBundles({
        provider,
        src_dir: srcDir,
        codex_dir: codexDir,
        claude_dir: claudeDir,
        mode,
        make_visible: provider === "codex" ? makeVisible : false,
        strict,
      });
      const ok = r.filter((x) => x.ok).length;
      const skipped = r.filter((x) => x.skipped_reason).length;
      const fail = r.filter((x) => x.error).length;
      const shaBad = r.filter((x) => x.sha_mismatch).length;
      toast.success(
        `导入完成：${ok}/${r.length}${skipped ? ` · ${skipped} 条跳过` : ""}${fail ? ` · ${fail} 条失败` : ""}${shaBad ? ` · ${shaBad} 条 sha 不一致` : ""}`,
      );
      await rescan(srcDir);
    } catch (e) {
      toast.error(String((e as Error)?.message ?? e));
    } finally {
      setRunning(false);
    }
  };

  const verified = items.filter((x) => x.verified === true).length;
  const corrupt = items.filter((x) => x.verified === false).length;

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="pb-3">
          <CardTitle className="flex items-center gap-2 text-base">
            <FolderOpen className="h-4 w-4" />
            数据源
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="space-y-1.5">
            <Label className="text-xs">数据文件夹（如果是跨设备，请先将 zip 文件解压后再选择里面的文件夹）</Label>
            <div className="flex gap-1.5">
              <Input
                value={srcDir}
                onChange={(e) => setSrcDir(e.target.value)}
                placeholder="选择包含 manifest.json 的解压文件夹"
                className="font-mono text-xs"
              />
              <Button variant="outline" size="sm" onClick={pickDir} className="shrink-0">
                浏览目录
              </Button>
              <Button variant="outline" size="sm" onClick={pickZip} className="shrink-0 gap-1.5">
                <FileArchive className="h-3.5 w-3.5" /> 导入 zip
              </Button>
            </div>
          </div>
          <Separator />
          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            <div className="space-y-1.5">
              <Label className="text-xs">冲突策略</Label>
              <RadioGroup
                value={mode}
                onValueChange={(v) => setMode(v as ImportMode)}
                className="flex flex-col gap-1"
              >
                <div className="flex items-center gap-2">
                  <RadioGroupItem value="skip" id="m-skip" />
                  <Label htmlFor="m-skip" className="text-xs font-normal">
                    跳过已存在的记录（如果本地已有同名会话则不导入，推荐使用）
                  </Label>
                </div>
                <div className="flex items-center gap-2">
                  <RadioGroupItem value="keep_local" id="m-keep" />
                  <Label htmlFor="m-keep" className="text-xs font-normal">
                    保留较新版本（如果本地已有且更新时间较晚，则保留本地，否则用导入的版本覆盖）
                  </Label>
                </div>
                <div className="flex items-center gap-2">
                  <RadioGroupItem value="overwrite" id="m-over" />
                  <Label htmlFor="m-over" className="text-xs font-normal">
                    强制覆盖（不管本地记录的新旧，直接用导入的数据予以替换）
                  </Label>
                </div>
              </RadioGroup>
            </div>
            <div className="space-y-3">
              {provider === "codex" && (
                <div className="space-y-1.5">
                  <div className="flex items-center gap-2">
                    <Switch
                      id="visible"
                      checked={makeVisible}
                      onCheckedChange={setMakeVisible}
                    />
                    <Label htmlFor="visible" className="cursor-pointer text-xs">
                      导入完成后，在应用内立刻显示这些会话内容
                    </Label>
                  </div>
                  <div className="pl-11 text-[11px] text-muted-foreground">
                    开启后会自动更新数据库和索引，保证记录刷新。如果关闭，仅将文件放入本地目录中，应用内可能暂时不可见。
                  </div>
                </div>
              )}
              <div className="space-y-1.5">
                <div className="flex items-center gap-2">
                  <Switch id="strict" checked={strict} onCheckedChange={setStrict} />
                  <Label htmlFor="strict" className="cursor-pointer text-xs">
                    严格 sha256 校验（推荐）
                  </Label>
                </div>
                <div className="pl-11 text-[11px] text-muted-foreground">
                  开启后：损坏的数据会被跳过。关闭后：即使校验失败也强制导入（除非你确定自己修改过内容，否则不建议关闭此项）。
                </div>
              </div>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            {items.length > 0 && (
              <div className="flex items-center gap-2 text-xs">
                {corrupt === 0 ? (
                  <CheckCircle2 className="h-3.5 w-3.5 text-emerald-500" />
                ) : (
                  <ShieldAlert className="h-3.5 w-3.5 text-amber-500" />
                )}
                <span>
                  {items.length} 条数据 · 校验 {verified} 通过
                  {corrupt ? ` · ${corrupt} 失败` : ""}
                </span>
              </div>
            )}
            <div className="ml-auto flex gap-1.5">
              <Button
                variant="outline"
                size="sm"
                onClick={() => rescan(srcDir)}
                disabled={!srcDir || scanLoading}
                className="gap-1.5"
              >
                重新扫描
              </Button>
              <Button
                size="sm"
                onClick={runImport}
                disabled={running || items.length === 0}
                className="gap-1.5"
              >
                <Download className="h-3.5 w-3.5" />
                执行导入
              </Button>
            </div>
          </div>
          {(running || scanLoading) && (
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              {running ? "执行中…" : "扫描中…"}
            </div>
          )}
        </CardContent>
      </Card>

      {items.length > 0 && (
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="flex items-center gap-2 text-base">
              待导入列表
              <Badge variant="secondary" className="h-5 px-1.5 font-normal">
                {items.length}
              </Badge>
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="rounded-md border">
              <div className="grid grid-cols-[8rem_minmax(0,1fr)_8rem_7rem_9rem_4rem] items-center gap-2 border-b bg-muted/40 px-3 py-2 text-[11px] font-medium text-muted-foreground">
                <span>id</span>
                <span>标题</span>
                <span>源设备(machine)</span>
                <span>服务商(provider)</span>
                <span>导出时间</span>
                <span>校验</span>
              </div>
              <ScrollArea className="h-96">
                <ul className="divide-y text-xs">
                  {items.map((it) => (
                    <li
                      key={it.bundle_dir}
                      className="grid grid-cols-[8rem_minmax(0,1fr)_8rem_7rem_9rem_4rem] items-center gap-2 px-3 py-2 hover:bg-muted/20"
                    >
                      <Tooltip>
                        <TooltipTrigger asChild>
                          <button
                            type="button"
                            onClick={async () => {
                              try {
                                await navigator.clipboard.writeText(it.manifest.session_id);
                                toast.success("已复制到剪贴板");
                              } catch {
                                toast.error("复制失败");
                              }
                            }}
                            className="flex min-w-0 items-center gap-1 truncate text-left font-mono text-[11px] text-muted-foreground hover:text-foreground"
                            aria-label="复制会话 ID"
                          >
                            <span className="truncate">
                              {it.manifest.session_id.slice(0, 8)}…
                            </span>
                            <Copy className="h-3 w-3 shrink-0 opacity-60" />
                          </button>
                        </TooltipTrigger>
                        <TooltipContent className="font-mono text-[11px]">
                          {it.manifest.session_id} · 点击复制
                        </TooltipContent>
                      </Tooltip>
                      <span
                        className="truncate"
                        title={it.manifest.thread_name || ""}
                      >
                        {it.manifest.thread_name || "—"}
                      </span>
                      <span
                        className="truncate text-muted-foreground"
                        title={it.manifest.export_machine}
                      >
                        {it.manifest.export_machine}
                      </span>
                      <span
                        className="truncate text-muted-foreground"
                        title={it.manifest.model_provider ?? ""}
                      >
                        {it.manifest.model_provider ?? "—"}
                      </span>
                      <span
                        className="truncate text-muted-foreground"
                        title={it.manifest.exported_at}
                      >
                        {it.manifest.exported_at}
                      </span>
                      <span>
                        {it.verified === true ? (
                          <Badge
                            variant="outline"
                            className="h-5 border-emerald-500/30 px-1.5 text-emerald-600"
                          >
                            OK
                          </Badge>
                        ) : it.verified === false ? (
                          <Badge
                            variant="outline"
                            className="h-5 border-rose-500/30 px-1.5 text-rose-500"
                          >
                            损坏
                          </Badge>
                        ) : (
                          <Badge
                            variant="outline"
                            className="h-5 px-1.5 text-muted-foreground"
                          >
                            ?
                          </Badge>
                        )}
                      </span>
                    </li>
                  ))}
                </ul>
              </ScrollArea>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
