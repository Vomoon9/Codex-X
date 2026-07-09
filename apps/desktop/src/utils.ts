import type { InstructionTemplate, SavedProvider, SavedPrompt, CodexState } from "./types";

export const LANG_KEY = "codexx.lang";
export const FALLBACK_GITHUB_REPO = "yynxxxxx/Codex-X";

export const instructionTemplates: InstructionTemplate[] = [
  {
    id: "gpt5.5-unrestricted",
    filename: "gpt5.5-unrestricted.md",
    title: "gpt-5.5 unrestricted",
    subtitle: "Codex-X 默认模板，适合 GPT-5.5 / Codex 5.5。",
    badge: "推荐",
  },
  {
    id: "gpt5.4-unrestricted",
    filename: "gpt5.4-unrestricted.md",
    title: "gpt-5.4 unrestricted",
    subtitle: "兼容旧版 GPT-5.4 / Codex 配置。",
    badge: "兼容",
  },
];

export const defaultProviderForm: SavedProvider = {
  id: "magicai",
  providerName: "MagicAI",
  baseUrl: "https://sky1818.com",
  model: "gpt-5.5",
  apiKey: "",
  wireApi: "responses",
  requiresOpenaiAuth: true,
};

export const blankProviderForm: SavedProvider = {
  id: "",
  providerName: "",
  baseUrl: "",
  model: "gpt-5.5",
  apiKey: "",
  wireApi: "responses",
  requiresOpenaiAuth: true,
};

export const blankPromptForm: SavedPrompt = {
  id: "",
  title: "",
  filename: "",
  content: "",
};

export function cx(...items: Array<string | false | undefined>) {
  return items.filter(Boolean).join(" ");
}

export function providerId(name: string) {
  const slug = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return slug || `provider-${Date.now()}`;
}

export function tomlEscape(value: string) {
  return value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

export function extractOpenAiApiKey(authText?: string) {
  if (!authText?.trim()) return "";
  try {
    const parsed = JSON.parse(authText) as { OPENAI_API_KEY?: unknown };
    return typeof parsed.OPENAI_API_KEY === "string" ? parsed.OPENAI_API_KEY : "";
  } catch {
    return "";
  }
}

export function buildProviderTomlPreview(provider: SavedProvider, state: CodexState | null) {
  const model = provider.model.trim() || "gpt-5.5";
  const name = provider.providerName.trim() || "your-provider";
  const baseUrl = provider.baseUrl.trim().replace(/\/+$/, "") || "https://example.com/v1";
  const wireApi = provider.wireApi || "responses";
  const source = state?.configText?.trimEnd() || "";
  const sourceLines = source ? source.split("\n") : [];
  const keptLines: string[] = [];
  let currentSection = "";
  let skippingCustomProvider = false;
  let hasReasoningEffort = false;

  for (const line of sourceLines) {
    const sectionMatch = line.match(/^\s*\[([^\]]+)]\s*$/);
    if (sectionMatch) {
      currentSection = sectionMatch[1].trim();
      skippingCustomProvider = currentSection === "model_providers.custom";
      if (skippingCustomProvider) continue;
    }
    if (skippingCustomProvider) continue;

    if (!currentSection) {
      const keyMatch = line.match(/^\s*([A-Za-z0-9_-]+)\s*=/);
      const key = keyMatch?.[1];
      if (key === "model_provider" || key === "model") continue;
      if (key === "model_reasoning_effort") hasReasoningEffort = true;
    }
    keptLines.push(line);
  }

  const firstSectionIndex = keptLines.findIndex((line) => /^\s*\[[^\]]+]\s*$/.test(line));
  const rootLines = (firstSectionIndex === -1 ? keptLines : keptLines.slice(0, firstSectionIndex)).filter((line, index, lines) => {
    if (line.trim()) return true;
    return index > 0 && index < lines.length - 1;
  });
  const sectionLines = firstSectionIndex === -1 ? [] : keptLines.slice(firstSectionIndex).filter((line, index, lines) => {
    if (line.trim()) return true;
    return index > 0 && index < lines.length - 1;
  });

  const headerLines = [
    'model_provider = "custom"',
    `model = "${tomlEscape(model)}"`,
  ];
  if (!hasReasoningEffort) {
    headerLines.push('model_reasoning_effort = "high"');
  }

  const providerLines = [
    "[model_providers.custom]",
    `name = "${tomlEscape(name)}"`,
    `base_url = "${tomlEscape(baseUrl)}"`,
    `wire_api = "${tomlEscape(wireApi)}"`,
    `requires_openai_auth = ${provider.requiresOpenaiAuth ? "true" : "false"}`,
  ];

  return [
    ...headerLines,
    ...(rootLines.length ? ["", ...rootLines] : []),
    "",
    ...providerLines,
    ...(sectionLines.length ? ["", ...sectionLines] : []),
  ].join("\n");
}

export function buildProviderAuthPreview(provider: SavedProvider) {
  const key = provider.apiKey?.trim();
  return JSON.stringify({ OPENAI_API_KEY: key || null }, null, 2);
}

export function instructionIdFromPath(path?: string) {
  if (!path) return "";
  const normalized = normalizeSlashes(path);
  const found = instructionTemplates.find((item) => normalized.endsWith(item.filename));
  return found?.id || "custom";
}

export function normalizeVersion(value?: string) {
  return (value || "").trim().replace(/^v/i, "");
}

export function compareVersions(a?: string, b?: string) {
  const pa = normalizeVersion(a).split(/[.-]/).map((x) => Number.parseInt(x, 10) || 0);
  const pb = normalizeVersion(b).split(/[.-]/).map((x) => Number.parseInt(x, 10) || 0);
  const len = Math.max(pa.length, pb.length, 3);
  for (let i = 0; i < len; i += 1) {
    const diff = (pa[i] || 0) - (pb[i] || 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

export function releaseAssetForPlatform(assets: Array<{ name?: string; browser_download_url?: string }>) {
  const platform = navigator.userAgent.toLowerCase();
  const isMac = platform.includes("mac");
  const isWindows = platform.includes("windows");
  const isLinux = platform.includes("linux");
  return assets.find((asset) => {
    const name = (asset.name || "").toLowerCase();
    if (isMac) return name.endsWith(".dmg") || name.endsWith(".app.tar.gz");
    if (isWindows) return name.endsWith(".msi") || name.endsWith(".exe");
    if (isLinux) return name.endsWith(".appimage") || name.endsWith(".deb") || name.endsWith(".rpm");
    return Boolean(name);
  }) || assets[0];
}

export function formatSessionTime(value?: number | null) {
  if (!value) return "未知时间";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "未知时间";
  return date.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function compactPath(value?: string | null, max = 58) {
  if (!value) return "未记录路径";
  const normalized = normalizeSlashes(value);
  if (normalized.length <= max) return normalized;
  const parts = normalized.split("/").filter(Boolean);
  if (parts.length >= 3) {
    const tail = parts.slice(-3).join("/");
    return `…/${tail}`;
  }
  return `…${normalized.slice(-max + 1)}`;
}

export function shortId(value: string) {
  return value.length > 8 ? value.slice(0, 8) : value;
}
export function normalizeSlashes(value?: string | null): string {
  if (!value) return "";
  return value.replace(/\\/g, "/");
}

export function isInstructionFile(instructionFile?: string, filename?: string): boolean {
  if (!instructionFile || !filename) return false;
  return normalizeSlashes(instructionFile).endsWith(filename);
}
