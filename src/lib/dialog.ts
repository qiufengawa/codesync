import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { isTauriRuntime } from "@/lib/runtime";

type FileFilter = {
  name: string;
  extensions: string[];
};

export async function pickDirectoryPath(options: {
  defaultPath?: string;
  title?: string;
  webPrompt?: string;
} = {}): Promise<string | null> {
  if (isTauriRuntime()) {
    const picked = await openDialog({
      directory: true,
      defaultPath: options.defaultPath,
      title: options.title,
    });
    return typeof picked === "string" ? picked : null;
  }

  return promptPath(
    options.webPrompt ??
      "浏览器 Web UI 不能打开系统目录选择器。请输入运行 cc-sessions webui 的那台机器上可访问的目录路径。",
    options.defaultPath,
  );
}

export async function pickFilePath(options: {
  defaultPath?: string;
  title?: string;
  filters?: FileFilter[];
  webPrompt?: string;
} = {}): Promise<string | null> {
  if (isTauriRuntime()) {
    const picked = await openDialog({
      defaultPath: options.defaultPath,
      title: options.title,
      filters: options.filters,
    });
    return typeof picked === "string" ? picked : null;
  }

  return promptPath(
    options.webPrompt ??
      "浏览器 Web UI 不能打开系统文件选择器。请输入运行 cc-sessions webui 的那台机器上可访问的文件路径。",
    options.defaultPath,
  );
}

export async function saveFilePath(options: {
  defaultPath?: string;
  title?: string;
  filters?: FileFilter[];
  webPrompt?: string;
} = {}): Promise<string | null> {
  if (isTauriRuntime()) {
    const picked = await saveDialog({
      defaultPath: options.defaultPath,
      title: options.title,
      filters: options.filters,
    });
    return typeof picked === "string" ? picked : null;
  }

  return promptPath(
    options.webPrompt ??
      "浏览器 Web UI 不能打开系统保存对话框。请输入运行 cc-sessions webui 的那台机器上要写入的文件路径。",
    options.defaultPath,
  );
}

function promptPath(message: string, defaultPath?: string): string | null {
  const picked = window.prompt(message, defaultPath ?? "");
  const trimmed = picked?.trim();
  return trimmed ? trimmed : null;
}
