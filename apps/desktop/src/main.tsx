import React from "react";
import ReactDOM from "react-dom/client";
import {
  CheckCircle2,
  ChevronRight,
  Code2,
  Download,
  ExternalLink,
  AlertCircle,
  FileCode2,
  Globe2,
  History,
  Info,
  KeyRound,
  Layers3,
  Loader2,
  Plus,
  Power,
  RefreshCw,
  RotateCcw,
  Settings,
  Sparkles,
  TerminalSquare,
  Trash2,
  Zap,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n, type Lang } from "./i18n";
import type {
  ProviderMode,
  InstructionMode,
  Tab,
  SavedProvider,
  SavedPrompt,
  BackupEntry,
  CodexState,
  ActionResult,
  ImportResult,
  OfficialAuthCandidate,
  AboutInfo,
  ReleaseInfo,
  SessionSyncStatus,
  SessionPreview,
  SessionSyncResult,
} from "./types";
import {
  cx,
  providerId,
  instructionIdFromPath,
  isInstructionFile,
  formatSessionTime,
  compactPath,
  shortId,
  releaseAssetForPlatform,
  compareVersions,
  buildProviderTomlPreview,
  buildProviderAuthPreview,
  extractOpenAiApiKey,
  LANG_KEY,
  FALLBACK_GITHUB_REPO,
  instructionTemplates,
  defaultProviderForm,
  blankProviderForm,
  blankPromptForm,
} from "./utils";
import {
  StatusPill,
  Field,
  StatCard,
  Avatar,
  OpenAIIcon,
  JsonPreview,
  TomlPreview,
} from "./components";
import "./styles.css";

function App() {
  const initialLang = (localStorage.getItem(LANG_KEY) as Lang | null) || "zh";
  const [lang, setLang] = React.useState<Lang>(initialLang === "en" ? "en" : "zh");
  const { t, i18n } = useI18n(lang);
  const isMacRuntime = navigator.userAgent.toLowerCase().includes("mac");
  const [tab, setTab] = React.useState<Tab>("dashboard");
  const [providerMode, setProviderMode] = React.useState<ProviderMode>("list");
  const [instructionMode, setInstructionMode] = React.useState<InstructionMode>("list");
  const [editingProviderId, setEditingProviderId] = React.useState<string | null>(null);
  const [editingPromptId, setEditingPromptId] = React.useState<string | null>(null);
  const [savedProviders, setSavedProviders] = React.useState<SavedProvider[]>([]);
  const [savedPrompts, setSavedPrompts] = React.useState<SavedPrompt[]>([]);
  const [aboutInfo, setAboutInfo] = React.useState<AboutInfo | null>(null);
  const [releaseInfo, setReleaseInfo] = React.useState<ReleaseInfo>({ status: "idle" });
  const [updatePromptOpen, setUpdatePromptOpen] = React.useState(false);
  const [sessionStatus, setSessionStatus] = React.useState<SessionSyncStatus | null>(null);
  const [state, setState] = React.useState<CodexState | null>(null);
  const [backups, setBackups] = React.useState<BackupEntry[]>([]);
  const [configDir, setConfigDir] = React.useState("");
  const configDirArg = configDir || null;
  const [loading, setLoading] = React.useState(false);
  const [toast, setToast] = React.useState<string>("");
  const [error, setError] = React.useState<string>("");
  const [providerForm, setProviderForm] = React.useState<SavedProvider>(defaultProviderForm);
  const [providerTomlDraft, setProviderTomlDraft] = React.useState("");
  const [providerTomlDirty, setProviderTomlDirty] = React.useState(false);
  const [promptForm, setPromptForm] = React.useState<SavedPrompt>(blankPromptForm);
  const [officialForm, setOfficialForm] = React.useState({ model: "gpt-5.5", authJson: "" });
  const autoUpdateCheckedRef = React.useRef(false);
  const providerTomlPreview = React.useMemo(() => buildProviderTomlPreview(providerForm, state), [providerForm, state]);
  const providerAuthPreview = React.useMemo(() => buildProviderAuthPreview(providerForm), [providerForm]);
  const currentInstructionId = instructionIdFromPath(state?.instructionFile);
    const releaseStatusLabel = React.useMemo(() => {
    if (releaseInfo.status === "checking") return i18n("检查中", "Checking");
    if (releaseInfo.status === "error") return i18n("失败", "Failed");
    if (releaseInfo.hasUpdate) return i18n("有更新", "Update found");
    if (releaseInfo.status === "ok") return i18n("已是最新", "Up to date");
    return i18n("未检查", "Idle");
  }, [i18n, releaseInfo.hasUpdate, releaseInfo.status]);

  React.useEffect(() => {
    localStorage.setItem(LANG_KEY, lang);
  }, [lang]);

  React.useEffect(() => {
    if (providerMode === "form" && !providerTomlDirty) {
      setProviderTomlDraft(providerTomlPreview);
    }
  }, [providerMode, providerTomlDirty, providerTomlPreview]);


  const currentProvider = state?.providers.find((p) => p.isCurrent);
  const detectedRows = React.useMemo(() => {
    return (state?.providers || []).map((p) => ({
      id: `detected-${p.id}`,
      source: "detected" as const,
      providerName: p.name || p.id,
      baseUrl: p.baseUrl || "",
      model: state?.model || "gpt-5.5",
      wireApi: p.wireApi || "responses",
      requiresOpenaiAuth: p.requiresOpenaiAuth ?? true,
      isCurrent: p.isCurrent,
    }));
  }, [state]);

  const localRows = React.useMemo(() => {
    return savedProviders.map((p) => ({
      ...p,
      source: "local" as const,
      isCurrent:
        Boolean(currentProvider) &&
        currentProvider?.baseUrl === p.baseUrl &&
        (state?.model || "") === p.model,
    }));
  }, [savedProviders, currentProvider, state?.model]);

  const providerRows = React.useMemo(() => {
    const officialRow = {
      id: "openai-official",
      source: "official" as const,
      providerName: "OpenAI Official",
      baseUrl: "https://chatgpt.com/codex",
      model: state?.model || "official",
      wireApi: "official",
      requiresOpenaiAuth: false,
      isCurrent: !state?.modelProvider || state.modelProvider === "openai",
    };
    const seen = new Set<string>();
    const rows: Array<typeof officialRow | (typeof detectedRows)[number] | (typeof localRows)[number]> = [officialRow];
    localRows.forEach((row) => {
      const key = `${row.baseUrl}::${row.model}`;
      if (key !== "::") seen.add(key);
      rows.push(row);
    });
    detectedRows.forEach((row) => {
      const key = `${row.baseUrl}::${row.model}`;
      if (key !== "::" && seen.has(key)) return;
      if (key !== "::") seen.add(key);
      rows.push(row);
    });
    return rows;
  }, [detectedRows, localRows, state?.model, state?.modelProvider]);

  const call = React.useCallback(async <T,>(fn: () => Promise<T>, success?: (data: T) => void) => {
    setLoading(true);
    setError("");
    try {
      const data = await fn();
      success?.(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const refresh = React.useCallback(() => {
    call(
      async () => {
        const next = await invoke<CodexState>("get_codex_state", { configDir: configDirArg });
        const backupList = await invoke<BackupEntry[]>("list_backups");
        const providerList = await invoke<SavedProvider[]>("list_saved_providers");
        const promptList = await invoke<SavedPrompt[]>("list_saved_prompts");
        const about = await invoke<AboutInfo>("get_about_info", { configDir: configDirArg });
        const sessions = await invoke<SessionSyncStatus>("get_session_sync_status", { configDir: configDirArg, targetProvider: null });
        return { next, backupList, providerList, promptList, about, sessions };
      },
      ({ next, backupList, providerList, promptList, about, sessions }) => {
        setState(next);
        setBackups(backupList);
        setSavedProviders(providerList);
        setSavedPrompts(promptList);
        setAboutInfo(about);
        setSessionStatus(sessions);
        if (!configDir) setConfigDir(next.codexDir);
      },
    );
  }, [call, configDir]);

  React.useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleActionResult = (result: ActionResult) => {
    setState(result.state);
    setToast(result.message);
    invoke<BackupEntry[]>("list_backups").then(setBackups).catch(() => undefined);
    invoke<SavedPrompt[]>("list_saved_prompts").then(setSavedPrompts).catch(() => undefined);
  };

  const action = (command: string, args: Record<string, unknown> = {}) =>
    call(() => invoke<ActionResult>(command, args), handleActionResult);

    const enableInstruction = () =>
    action("enable_instruction", { configDir: configDirArg });

    const switchInstructionTemplate = (templateId: string) =>
    action("enable_instruction_template", { configDir: configDirArg, templateId });

    const disableInstruction = () =>
    action("disable_instruction", { configDir: configDirArg, deleteFile: true });

    const setPromptFormMode = (prompt: SavedPrompt, editingId: string | null) => {
    setEditingPromptId(editingId);
    setPromptForm(prompt);
    setInstructionMode("form");
  };
  const openAddPrompt = () => setPromptFormMode({ ...blankPromptForm }, null);
  const openEditPrompt = (prompt: SavedPrompt) => setPromptFormMode(prompt, prompt.id);

  

  const normalizedPromptForm = (): SavedPrompt => ({
    ...promptForm,
    id: editingPromptId || promptForm.id || providerId(promptForm.title || promptForm.filename),
    title: promptForm.title.trim(),
    filename: promptForm.filename.trim(),
    content: promptForm.content,
  });

  const savePromptOnly = () =>
    call(
      async () => {
        await invoke<SavedPrompt>("save_prompt", { prompt: normalizedPromptForm() });
        return invoke<SavedPrompt[]>("list_saved_prompts");
      },
      (promptList) => {
        setSavedPrompts(promptList);
        setInstructionMode("list");
        setEditingPromptId(null);
        setToast(i18n("提示词已保存", "Prompt saved"));
      },
    );

  const saveAndEnablePrompt = () =>
    call(
      async () => {
        const saved = await invoke<SavedPrompt>("save_prompt", { prompt: normalizedPromptForm() });
        const result = await invoke<ActionResult>("enable_saved_prompt", { configDir: configDirArg, id: saved.id });
        const promptList = await invoke<SavedPrompt[]>("list_saved_prompts");
        return { result, promptList };
      },
      ({ result, promptList }) => {
        setSavedPrompts(promptList);
        setInstructionMode("list");
        setEditingPromptId(null);
        handleActionResult(result);
      },
    );

    const enableSavedPrompt = (id: string) =>
    action("enable_saved_prompt", { configDir: configDirArg, id });

  const removeSavedPrompt = (id: string) =>
    call(
      async () => {
        await invoke<void>("delete_saved_prompt", { id });
        return invoke<SavedPrompt[]>("list_saved_prompts");
      },
      (promptList) => {
        setSavedPrompts(promptList);
        setToast(i18n("提示词已删除", "Prompt deleted"));
      },
    );

  const normalizedProviderForm = (): SavedProvider => ({
    ...providerForm,
    id: editingProviderId || providerForm.id || providerId(providerForm.providerName || providerForm.baseUrl),
    providerName: providerForm.providerName.trim(),
    baseUrl: providerForm.baseUrl.trim().replace(/\/+$/, ""),
    model: providerForm.model.trim(),
    apiKey: (providerForm.apiKey || "").trim(),
    wireApi: providerForm.wireApi || "responses",
    requiresOpenaiAuth: providerForm.requiresOpenaiAuth,
  });

  const reloadSavedProviders = async () => {
    const providerList = await invoke<SavedProvider[]>("list_saved_providers");
    setSavedProviders(providerList);
    return providerList;
  };

  const saveProviderOnly = () =>
    call(
      async () => {
        const saved = await invoke<SavedProvider>("save_provider", { provider: normalizedProviderForm() });
        const providerList = await invoke<SavedProvider[]>("list_saved_providers");
        return { saved, providerList };
      },
      ({ providerList }) => {
        setSavedProviders(providerList);
        setProviderMode("list");
        setEditingProviderId(null);
        setToast(i18n("供应商已保存到 SQLite", "Provider saved to SQLite"));
      },
    );

    const switchProvider = (provider: SavedProvider) =>
    action("switch_provider", {
      input: {
        configDir: configDirArg,
        providerName: provider.providerName,
        baseUrl: provider.baseUrl,
        model: provider.model,
        apiKey: provider.apiKey || "",
        wireApi: provider.wireApi,
        requiresOpenaiAuth: provider.requiresOpenaiAuth,
      },
    });

  const saveAndSwitch = () =>
    call(
      async () => {
        const saved = await invoke<SavedProvider>("save_provider", { provider: normalizedProviderForm() });
        const result = await invoke<ActionResult>("save_provider_toml_config", {
          input: {
            configDir: configDirArg,
            configText: providerTomlDraft || buildProviderTomlPreview(saved, state),
            apiKey: saved.apiKey || "",
          },
        });
        const providerList = await invoke<SavedProvider[]>("list_saved_providers");
        return { result, providerList };
      },
      ({ result, providerList }) => {
        setSavedProviders(providerList);
        setProviderMode("list");
        setEditingProviderId(null);
        setProviderTomlDirty(false);
        handleActionResult(result);
      },
    );

    const switchOfficialProvider = () =>
    action("switch_official_provider", { configDir: configDirArg });

  const importFromCcSwitch = () =>
    call(
      () => invoke<ImportResult>("import_ccswitch_codex_providers", { dbPath: null }),
      (result) => {
        setSavedProviders(result.providers);
        const warningText = result.warnings.length > 0 ? `，跳过 ${result.skipped}` : "";
        setToast(
          i18n(`已从 cc-switch 导入 ${result.imported} 个供应商${warningText}`, `Imported ${result.imported} provider(s) from cc-switch${warningText}`),
        );
      },
    );

    const restoreBackup = (backupId: string) =>
    action("restore_backup", { configDir: configDirArg, backupId });

    const openExternalUrl = React.useCallback((url?: string | null) => {
    if (!url) return;
    window.setTimeout(() => {
      void invoke("open_url", { url }).catch(() => {
        setToast(i18n("打开浏览器失败", "Failed to open browser"));
      });
    }, 0);
  }, [i18n]);

  const checkForUpdates = React.useCallback(async ({ quiet = false }: { quiet?: boolean } = {}) => {
    const repo = aboutInfo?.githubRepo || FALLBACK_GITHUB_REPO;
    const appVersion = aboutInfo?.appVersion || "0.0.0";
    const releasesUrl = `https://github.com/${repo}/releases/`;
    setReleaseInfo({ status: "checking" });
    try {
      const response = await fetch(`https://api.github.com/repos/${repo}/releases/latest`, {
        headers: { Accept: "application/vnd.github+json" },
      });
      if (!response.ok) {
        throw new Error(`GitHub Releases ${response.status}`);
      }
      const release = await response.json() as {
        tag_name?: string;
        name?: string;
        html_url?: string;
        body?: string;
        assets?: Array<{ name?: string; browser_download_url?: string }>;
      };
      const latestVersion = release.tag_name || release.name || "";
      const asset = releaseAssetForPlatform(release.assets || []);
      const hasUpdate = compareVersions(latestVersion, appVersion) > 0;
      const message = hasUpdate
        ? (i18n("发现新版本", "Update available"))
        : (i18n("当前已是最新版本", "You are up to date"));
      setReleaseInfo({
        status: "ok",
        latestVersion,
        htmlUrl: releasesUrl,
        assetName: asset?.name,
        body: release.body || "",
        hasUpdate,
        message,
      });
      if (hasUpdate) {
        if (quiet) {
          setToast(i18n(`发现新版本 ${latestVersion}，可在概览页查看`, `New version ${latestVersion} is available`));
        } else {
          setUpdatePromptOpen(true);
        }
      } else if (!quiet) {
        setToast(message);
      }
    } catch (e) {
      const message = quiet ? (i18n("自动检查失败", "Auto check failed")) : (i18n("检查失败", "Check failed"));
      setReleaseInfo({
        status: "error",
        message,
      });
      if (!quiet) setToast(message);
    }
  }, [aboutInfo?.githubRepo, aboutInfo?.appVersion, i18n]);

  React.useEffect(() => {
    if (!state || !aboutInfo || autoUpdateCheckedRef.current) return;
    autoUpdateCheckedRef.current = true;
    void checkForUpdates({ quiet: true });
  }, [state, aboutInfo, checkForUpdates]);

  const loadCcSwitchOfficialAuth = async (showToast = true) => {
    const candidate = await invoke<OfficialAuthCandidate | null>("read_ccswitch_official_auth", { dbPath: null });
    if (candidate) {
      setOfficialForm({
        model: candidate.model || state?.model || "gpt-5.5",
        authJson: candidate.authJson,
      });
      if (showToast) setToast(t.provider.officialAuthLoaded);
      return true;
    }
    if (showToast) setToast(t.provider.officialAuthNotFound);
    return false;
  };

  const openOfficialEdit = () => {
    setOfficialForm({
      model: state?.model || "gpt-5.5",
      authJson: state?.authText || '{\n  "OPENAI_API_KEY": null,\n  "auth_mode": "chatgpt",\n  "tokens": {\n    "access_token": "",\n    "refresh_token": "",\n    "id_token": ""\n  }\n}',
    });
    setProviderMode("official");
    void loadCcSwitchOfficialAuth(false);
  };

    const saveOfficialConfig = () =>
    action("save_official_config", {
      input: {
        configDir: configDirArg,
        model: officialForm.model,
        authJson: officialForm.authJson,
      },
    });

    const setProviderFormMode = (provider: SavedProvider, editingId: string | null) => {
    setEditingProviderId(editingId);
    setProviderForm(provider);
    setProviderTomlDraft(buildProviderTomlPreview(provider, state));
    setProviderTomlDirty(false);
    setProviderMode("form");
  };
  const openAddProvider = () => setProviderFormMode({ ...blankProviderForm }, null);
  const openEditProvider = (provider: SavedProvider) => setProviderFormMode(provider, provider.id);
  const openEditDetectedProvider = (provider: { id: string; providerName: string; baseUrl: string; model: string; wireApi: string; requiresOpenaiAuth: boolean }) => {
    const id = providerId(provider.providerName || provider.baseUrl);
    const next: SavedProvider = {
      id,
      providerName: provider.providerName,
      baseUrl: provider.baseUrl,
      model: provider.model,
      apiKey: extractOpenAiApiKey(state?.authText),
      wireApi: provider.wireApi || "responses",
      requiresOpenaiAuth: provider.requiresOpenaiAuth,
    };
    setProviderFormMode(next, id);
  };

  

  

  const removeProvider = (id: string) => {
    call(
      async () => {
        await invoke<void>("delete_saved_provider", { id });
        return invoke<SavedProvider[]>("list_saved_providers");
      },
      (providerList) => {
        setSavedProviders(providerList);
        setToast(i18n("已从 SQLite 删除供应商", "Provider deleted from SQLite"));
      },
    );
  };

  const checkSessions = () =>
    call(
      () => invoke<SessionSyncStatus>("get_session_sync_status", { configDir: configDirArg, targetProvider: null }),
      (status) => {
        setSessionStatus(status);
        setToast(status.needsSync
          ? (i18n(`发现 ${status.mismatchedSessionMeta + status.mismatchedThreads} 项需要同步`, "Session sync needed"))
          : (i18n("会话已同步", "Sessions are in sync")));
      },
    );

  const syncSessions = () =>
    call(
      () => invoke<SessionSyncResult>("sync_sessions_provider", { configDir: configDirArg, targetProvider: null }),
      (result) => {
        setSessionStatus(result.status);
        setToast(i18n(`已修复 ${result.updatedRollouts} 个会话文件、${result.updatedThreads} 条 SQLite 记录`, `Updated ${result.updatedRollouts} rollout file(s), ${result.updatedThreads} SQLite row(s)`));
      },
    );

  const navItems: Array<[Tab, string, React.ReactNode]> = [
    ["dashboard", t.nav.dashboard, <Layers3 size={18} />],
    ["provider", t.nav.provider, <Zap size={18} />],
    ["sessions", t.nav.sessions, <History size={18} />],
    ["instruction", t.nav.instruction, <Sparkles size={18} />],
    ["toml", t.nav.toml, <FileCode2 size={18} />],
    ["settings", t.nav.settings, <Settings size={18} />],
    ["about", t.nav.about, <Info size={18} />],
  ];

  return (
    <main className={cx("app-shell", isMacRuntime && "mac-shell")}>
      {isMacRuntime && <div className="window-drag-strip" data-tauri-drag-region />}
      <div className="orb orb-a" />
      <div className="orb orb-b" />

      <aside className="sidebar glass">
        <div className="brand">
          <div className="brand-mark">X</div>
          <div>
            <h1>Codex-X</h1>
            <p>{t.appSubtitle}</p>
          </div>
        </div>

        <nav>
          {navItems.map(([id, label, icon]) => (
            <button key={id} className={cx("nav-item", tab === id && "active")} onClick={() => setTab(id)}>
              {icon}
              <span>{label}</span>
              {tab === id && <ChevronRight size={16} />}
            </button>
          ))}
        </nav>

        <div className="sidebar-footer" />
      </aside>

      <section className="content">
        {tab === "dashboard" && (
          <header className="topbar glass">
            <div>
              <p className="eyebrow">{t.manager}</p>
              <h2>{state?.model || "gpt-5.5"}</h2>
            </div>
            <div className="path-box">
              <span>CODEX_HOME</span>
              <input value={configDir} onChange={(e) => setConfigDir(e.target.value)} placeholder="~/.codex" />
            </div>
            <button className="primary-btn" onClick={refresh} disabled={loading}>
              {loading ? <Loader2 className="spin" size={17} /> : <RefreshCw size={17} />}
              {t.load}
            </button>
          </header>
        )}

        {toast && (
          <div className="toast ok" onAnimationEnd={() => setToast("")}>
            <CheckCircle2 size={18} /> {toast}
          </div>
        )}
        {error && <div className="toast error">{error}</div>}
        {updatePromptOpen && releaseInfo.hasUpdate && (
          <div className="update-mask" onClick={() => setUpdatePromptOpen(false)}>
            <div className="update-dialog glass" onClick={(e) => e.stopPropagation()}>
              <div className="update-head">
                <div className="update-icon"><Sparkles size={22} /></div>
                <div>
                  <p className="eyebrow">Codex-X</p>
                  <h3>{i18n("发现新版本", "New version available")}</h3>
                </div>
              </div>
              <div className="update-body">
                <p>{i18n("检测到新版本，是否立即打开下载页？", "A new version was found. Open the download page now?")}</p>
                <div className="about-kv compact">
                  <div><span>{i18n("当前版本", "Current")}</span><strong>{aboutInfo?.appVersion || "-"}</strong></div>
                  <div><span>{i18n("最新版本", "Latest")}</span><strong>{releaseInfo.latestVersion || "-"}</strong></div>
                </div>
              </div>
              <div className="update-actions">
                <button className="primary-btn" onClick={() => {
                  setUpdatePromptOpen(false);
                  openExternalUrl(releaseInfo.htmlUrl);
                }}>
                  <Download size={16} /> {i18n("现在下载", "Download now")}
                </button>
                <button className="secondary-btn" onClick={() => setUpdatePromptOpen(false)}>
                  {i18n("稍后", "Later")}
                </button>
              </div>
            </div>
          </div>
        )}

        {!state ? (
          <div className="panel glass center-panel">
            <Loader2 className="spin" />
            <p>{t.loadingConfig}</p>
          </div>
        ) : (
          <>
            {tab === "dashboard" && (
              <>
                {releaseInfo.status === "ok" && releaseInfo.hasUpdate && (
                  <div className="update-strip glass">
                    <div>
                      <span className="update-dot" />
                      <strong>{i18n("发现新版本", "New version found")}</strong>
                      <p>{i18n(`Codex-X ${releaseInfo.latestVersion || ""} 已发布`, `Codex-X ${releaseInfo.latestVersion || ""} is available`)}</p>
                    </div>
                    <button className="secondary-btn small" onClick={() => openExternalUrl(releaseInfo.htmlUrl)}>
                      {i18n("查看更新", "View")}
                    </button>
                  </div>
                )}
              <div className="grid dashboard-grid">
                <StatCard icon={<TerminalSquare size={20} />} label={t.dashboard.config} value={state.configExists ? t.dashboard.found : t.dashboard.missing} ok={state.configExists} />
                <StatCard icon={<Code2 size={20} />} label={t.dashboard.provider} value={currentProvider?.name || state.modelProvider || t.dashboard.officialDefault} ok={Boolean(state.modelProvider)} />
                <StatCard icon={<Sparkles size={20} />} label={t.dashboard.instruction} value={state.instructionEnabled ? t.dashboard.enabled : t.dashboard.disabled} ok={state.instructionEnabled} />
                <StatCard icon={<KeyRound size={20} />} label={t.dashboard.auth} value={state.authExists ? t.authJson : t.noAuth} ok={state.authExists} />

                <section className="panel glass wide">
                  <div className="panel-head">
                    <div>
                      <p className="eyebrow">{t.dashboard.liveStatus}</p>
                      <h3>{t.dashboard.currentConfig}</h3>
                    </div>
                    <StatusPill active={state.instructionEnabled} label={state.instructionEnabled ? "Instruct ON" : "Instruct OFF"} />
                  </div>
                  <div className="kv-list">
                    <div><span>{t.dashboard.dir}</span><code>{state.codexDir}</code></div>
                    <div><span>{t.dashboard.configPath}</span><code>{state.configPath}</code></div>
                    <div><span>{t.dashboard.model}</span><code>{state.model || t.dashboard.notSet}</code></div>
                    <div><span>{t.dashboard.providerName}</span><code>{state.modelProvider || t.dashboard.officialDefault}</code></div>
                    <div><span>{t.dashboard.instructionFile}</span><code>{state.instructionFile || t.dashboard.notSet}</code></div>
                  </div>
                </section>

                <section className="panel glass">
                  <div className="panel-head compact"><h3>{t.dashboard.quickActions}</h3></div>
                  <div className="action-stack">
                    <button className="primary-btn big" onClick={enableInstruction} disabled={loading}><Power size={18} /> {t.dashboard.enableInstruction}</button>
                    <button className="secondary-btn big" onClick={disableInstruction} disabled={loading}><RotateCcw size={18} /> {t.dashboard.disableInstruction}</button>
                    {backups[0] && <button className="ghost-btn big" onClick={() => restoreBackup(backups[0].id)} disabled={loading}><History size={18} /> {t.dashboard.restoreLatest}</button>}
                  </div>
                </section>
              </div>
              </>
            )}

            {tab === "provider" && (
              <section className={cx("panel glass provider-panel", providerMode !== "list" && "provider-edit-panel")}> 
                {providerMode === "list" ? (
                  <>
                    <div className="panel-head provider-title-row">
                      <div>
                        <p className="eyebrow">Provider</p>
                        <h3>{t.provider.title}</h3>
                        <p className="muted-desc">{t.provider.subtitle}</p>
                      </div>
                      <div className="provider-title-actions">
                        <button className="secondary-btn add-provider-btn" onClick={importFromCcSwitch} disabled={loading}><RefreshCw size={18} /> {t.provider.importCc}</button>
                        <button className="primary-btn add-provider-btn" onClick={openAddProvider}><Plus size={18} /> {t.provider.add}</button>
                      </div>
                    </div>

                    <div className="provider-row-list">
                      {providerRows.length === 0 && <p className="empty">{t.provider.noProviders}</p>}
                      {providerRows.map((p) => {
                        const local = p.source === "official"
                          ? undefined
                          : savedProviders.find((item) =>
                            p.source === "local"
                              ? item.id === p.id
                              : item.baseUrl === p.baseUrl && item.model === p.model,
                          );
                        const switchable: SavedProvider | null = p.source === "official" ? null : local || {
                          id: providerId(p.providerName),
                          providerName: p.providerName,
                          baseUrl: p.baseUrl,
                          model: p.model,
                          apiKey: "",
                          wireApi: p.wireApi,
                          requiresOpenaiAuth: p.requiresOpenaiAuth,
                        };
                        return (
                          <div className={cx("provider-row", p.isCurrent && "selected")} key={`${p.source}-${p.id}-${p.baseUrl}`}>
                            <div className="drag-dot">⋮⋮</div>
                            {p.source === "official" ? <OpenAIIcon /> : <Avatar name={p.providerName} />}
                            <div className="provider-main">
                              <strong>{p.providerName}</strong>
                              <a>{p.baseUrl || "no base_url"}</a>
                            </div>
                            <div className="provider-meta">
                              <span>{p.source === "official" ? t.provider.official : p.source === "detected" ? t.provider.detected : t.provider.local}</span>
                              <code>{p.model}</code>
                            </div>
                            <div className="provider-badges">
                              {p.isCurrent && <StatusPill active label={t.provider.current} />}
                              {p.source === "official" && <StatusPill active={false} label={t.provider.noRouting} />}
                              {p.source === "official" && <StatusPill active={state.officialAuthAvailable} label={state.officialAuthAvailable ? t.provider.authReady : t.provider.authMissing} />}
                            </div>
                            <div className="provider-actions">
                              <button className="secondary-btn small" onClick={() => switchable ? switchProvider(switchable) : switchOfficialProvider()} disabled={loading || p.isCurrent}>{t.provider.switch}</button>
                              {p.source === "official" && <button className="ghost-btn small" onClick={openOfficialEdit}>{t.provider.viewEdit}</button>}
                              {local && <button className="ghost-btn small" onClick={() => openEditProvider(local)}>{t.provider.edit}</button>}
                              {!local && p.source === "detected" && <button className="ghost-btn small" onClick={() => openEditDetectedProvider(p)}>{t.provider.edit}</button>}
                              {local && <button className="danger-btn small" onClick={() => removeProvider(local.id)}><Trash2 size={14} /> {t.provider.remove}</button>}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </>
                ) : providerMode === "official" ? (
                  <div className="provider-form-page">
                    <div className="panel-head">
                      <div>
                        <p className="eyebrow">OpenAI Official</p>
                        <h3>{t.provider.officialEdit}</h3>
                        <p className="muted-desc">{t.provider.officialHint}</p>
                      </div>
                      <button className="ghost-btn" onClick={() => setProviderMode("list")}>{t.provider.cancel}</button>
                    </div>
                    <div className="official-info-card">
                      <div><span>{t.provider.officialUrl}</span><code>https://chatgpt.com/codex</code></div>
                      <div><span>auth.json</span><code>{state.authPath}</code></div>
                      <div><span>{t.provider.current}</span><code>{(!state.modelProvider || state.modelProvider === "openai") ? "OpenAI Official" : state.modelProvider}</code></div>
                    </div>
                    <div className="form-grid provider-form-grid">
                      <Field label={t.provider.model}><input value={officialForm.model} onChange={(e) => setOfficialForm({ ...officialForm, model: e.target.value })} /></Field>
                    </div>
                    <label className="field official-auth-field">
                      <span>auth.json (JSON)</span>
                      <textarea className="official-auth-editor" value={officialForm.authJson} onChange={(e) => setOfficialForm({ ...officialForm, authJson: e.target.value })} spellCheck={false} />
                    </label>
                    <div className="form-actions">
                      <button className="ghost-btn big" onClick={() => void loadCcSwitchOfficialAuth(true)} disabled={loading}>{t.provider.loadOfficialAuth}</button>
                      <button className="secondary-btn big" onClick={() => setProviderMode("list")}>{t.provider.cancel}</button>
                      <button className="primary-btn big" onClick={saveOfficialConfig} disabled={loading}>保存官方配置</button>
                    </div>
                  </div>
                ) : (
                  <div className="provider-form-page">
                    <div className="panel-head">
                      <div>
                        <p className="eyebrow">Provider</p>
                        <h3>{editingProviderId ? t.provider.formEdit : t.provider.formAdd}</h3>
                        <p className="muted-desc">{t.provider.formHint}</p>
                      </div>
                      <button className="ghost-btn" onClick={() => setProviderMode("list")}>{t.provider.cancel}</button>
                    </div>
                    <div className="provider-edit-stack">
                      <section className="provider-section provider-api-section unified-section">
                        <div className="section-title-row">
                          <div>
                            <strong>{i18n("供应商 API 配置", "Provider API config")}</strong>
                            <p>{i18n("和 cc-switch 一样，API 信息、auth.json、config.toml 在同一个编辑页纵向展示。", "API fields, auth.json and config.toml are shown vertically in one edit page.")}</p>
                          </div>
                        </div>
                        <div className="form-grid provider-form-grid provider-form-cc">
                          <Field label={t.provider.apiKey}><input type="password" value={providerForm.apiKey || ""} onChange={(e) => setProviderForm({ ...providerForm, apiKey: e.target.value })} placeholder={t.provider.apiKeyPlaceholder} /></Field>
                          <Field label={i18n("API 请求地址", t.provider.baseUrl)}><input value={providerForm.baseUrl} onChange={(e) => setProviderForm({ ...providerForm, baseUrl: e.target.value })} /></Field>
                          <Field label={t.provider.name}><input value={providerForm.providerName} onChange={(e) => setProviderForm({ ...providerForm, providerName: e.target.value, id: editingProviderId || providerId(e.target.value) })} /></Field>
                          <Field label={t.provider.model}><input value={providerForm.model} onChange={(e) => setProviderForm({ ...providerForm, model: e.target.value })} /></Field>
                          <Field label={t.provider.wireApi}>
                            <select value={providerForm.wireApi} onChange={(e) => setProviderForm({ ...providerForm, wireApi: e.target.value })}>
                              <option value="responses">responses</option>
                              <option value="chat">chat</option>
                            </select>
                          </Field>
                          <label className="check-row"><input type="checkbox" checked={providerForm.requiresOpenaiAuth} onChange={(e) => setProviderForm({ ...providerForm, requiresOpenaiAuth: e.target.checked })} /><span>{t.provider.requiresAuth}</span></label>
                        </div>
                      </section>

                      <section className="provider-section provider-auth-section unified-section">
                        <div className="section-title-row">
                          <div>
                            <strong>auth.json (JSON)</strong>
                            <p>{i18n("预览保存时会写入/保留的认证配置；API Key 留空时不会覆盖现有 auth.json。", "Preview of auth config. Empty API key will not overwrite the existing auth.json.")}</p>
                          </div>
                        </div>
                        <JsonPreview text={providerAuthPreview} />
                      </section>

                      <section className="provider-section provider-toml-section unified-section">
                        <div className="section-title-row config-title-row">
                          <div>
                            <strong>config.toml (TOML)</strong>
                            <p>{i18n("可直接编辑，保存时会写入 Codex live config.toml。", "Editable. Saved directly to the Codex live config.toml.")}</p>
                          </div>
                          <button className="ghost-btn small" onClick={() => { setProviderTomlDraft(providerTomlPreview); setProviderTomlDirty(false); }}>{i18n("重置生成", "Reset")}</button>
                        </div>
                        <textarea
                          className="provider-toml-editor"
                          value={providerTomlDraft}
                          onChange={(e) => { setProviderTomlDraft(e.target.value); setProviderTomlDirty(true); }}
                          spellCheck={false}
                        />
                      </section>

                      <div className="form-actions provider-save-actions">
                        <button className="primary-btn big" onClick={saveAndSwitch} disabled={loading}><Zap size={18} /> {t.provider.saveAndSwitch}</button>
                      </div>
                    </div>
                  </div>
                )}
              </section>
            )}

            {tab === "sessions" && (
              <section className="panel glass sessions-panel">
                <div className="panel-head provider-title-row">
                  <div>
                    <p className="eyebrow">Provider Sync</p>
                    <h3>{i18n("会话管理", "Session management")}</h3>
                    <p className="muted-desc">
                      {i18n("检查并修复 Codex 本地历史会话的 Provider 元数据，让切换供应商后旧 thread 仍能被原生 Codex 识别、打开和续聊。", "Check and repair local Codex session provider metadata so old threads stay visible and resumable after provider switching.")}
                    </p>
                  </div>
                  <div className="provider-title-actions">
                    <button className="secondary-btn add-provider-btn" onClick={checkSessions} disabled={loading}>
                      <RefreshCw size={18} className={cx(loading && "spin")} /> {i18n("检查会话", "Check")}
                    </button>
                    <button className="primary-btn add-provider-btn" onClick={syncSessions} disabled={loading || !sessionStatus?.needsSync}>
                      <Zap size={18} /> {i18n("同步 / 修复", "Sync / repair")}
                    </button>
                  </div>
                </div>

                <div className={cx("session-status-card", sessionStatus?.needsSync ? "needs-sync" : "synced")}>
                  <div className="session-status-icon">
                    {sessionStatus?.needsSync ? <AlertCircle size={24} /> : <CheckCircle2 size={24} />}
                  </div>
                  <div>
                    <strong>{sessionStatus?.needsSync ? (i18n("发现未同步会话", "Unsynced sessions found")) : (i18n("会话已同步", "Sessions are in sync"))}</strong>
                    <p>{i18n(`目标 Provider：${sessionStatus?.targetProvider || state.modelProvider || "openai"}`, `Target provider: ${sessionStatus?.targetProvider || state.modelProvider || "openai"}`)}</p>
                  </div>
                  {sessionStatus?.needsSync && <StatusPill active label={i18n("需要修复", "Needs repair")} />}
                </div>

                <div className="session-stat-grid">
                  <StatCard icon={<FileCode2 size={20} />} label={i18n("rollout 文件", "Rollout files")} value={sessionStatus?.rolloutFiles ?? "-"} ok />
                  <StatCard icon={<Sparkles size={20} />} label={i18n("session_meta", "session_meta")} value={sessionStatus?.sessionMetaCount ?? "-"} ok />
                  <StatCard icon={<AlertCircle size={20} />} label={i18n("未同步 JSONL", "Unsynced JSONL")} value={sessionStatus?.mismatchedRollouts ?? "-"} ok={!sessionStatus?.mismatchedRollouts} />
                  <StatCard icon={<Layers3 size={20} />} label={i18n("SQLite threads", "SQLite threads")} value={sessionStatus?.sqliteThreads ?? "-"} ok />
                  <StatCard icon={<AlertCircle size={20} />} label={i18n("未同步记录", "Unsynced rows")} value={sessionStatus?.mismatchedThreads ?? "-"} ok={!sessionStatus?.mismatchedThreads} />
                  <StatCard icon={<Code2 size={20} />} label={i18n("SQLite 数据库", "SQLite DBs")} value={sessionStatus?.sqliteDbs ?? "-"} ok />
                </div>


                <div className="session-list-card">
                  <div className="session-list-head">
                    <div>
                      <p className="eyebrow">{i18n("本地会话", "Local threads")}</p>
                      <h4>{i18n("会话列表", "Sessions")}</h4>
                    </div>
                    <span>{i18n(`展示 ${sessionStatus?.sessions?.length ?? 0} / ${sessionStatus?.sqliteThreads ?? 0} 条`, `${sessionStatus?.sessions?.length ?? 0} / ${sessionStatus?.sqliteThreads ?? 0} shown`)}</span>
                  </div>
                  {sessionStatus?.sessions?.length ? (
                    <div className="session-list">
                      {sessionStatus.sessions.map((item) => (
                        <article className={cx("session-row", item.needsSync && "needs-sync")} key={item.id}>
                          <div className="session-row-main">
                            <div className="session-thread-icon"><History size={18} /></div>
                            <div className="session-row-text">
                              <div className="session-row-title">
                                <strong>{item.title}</strong>
                                {item.archived && <span className="mini-tag">{i18n("已归档", "Archived")}</span>}
                                {item.needsSync && <span className="mini-tag warn">{i18n("需同步", "Needs sync")}</span>}
                              </div>
                              <p title={item.cwd || item.rolloutPath || undefined}>{compactPath(item.cwd || item.rolloutPath)}</p>
                            </div>
                          </div>
                          <div className="session-row-meta">
                            <span>{formatSessionTime(item.updatedAtMs)}</span>
                            <code>{item.modelProvider || "unknown"}</code>
                            <em>{item.model || "model -"}</em>
                            <small>#{shortId(item.id)}</small>
                          </div>
                        </article>
                      ))}
                    </div>
                  ) : (
                    <div className="session-empty">
                      <History size={22} />
                      <span>{i18n("还没有读取到会话。点击右上角“检查会话”刷新。", "No sessions loaded. Click Check to refresh.")}</span>
                    </div>
                  )}
                </div>

                {sessionStatus?.warnings?.length ? (
                  <div className="session-warning-list">
                    {sessionStatus.warnings.map((item, index) => <p key={index}><AlertCircle size={15} /> {item}</p>)}
                  </div>
                ) : null}
              </section>
            )}

            {tab === "instruction" && (
              <section className="panel glass instruction-panel simple-instruction-panel">
                {instructionMode === "list" ? (
                  <>
                    <div className="panel-head provider-title-row">
                      <div>
                        <p className="eyebrow">model_instructions_file</p>
                        <h3>{t.instruction.title}</h3>
                      </div>
                      <div className="provider-title-actions">
                        <button className="primary-btn add-provider-btn" onClick={openAddPrompt}><Plus size={18} /> {i18n("添加提示词", "Add prompt")}</button>
                      </div>
                    </div>

                    <div className="instruction-row-list">
                      {instructionTemplates.map((item) => {
                        const isCurrent = currentInstructionId === item.id;
                        return (
                          <div className={cx("instruction-row", isCurrent && "selected")} key={item.id}>
                            <div className="instruction-icon"><Sparkles size={22} /></div>
                            <div className="instruction-main">
                              <strong>{item.title}</strong>
                              <p>{item.subtitle}</p>
                              <code>./{item.filename}</code>
                            </div>
                            <div className="instruction-status-col">
                              {isCurrent ? <StatusPill active label={t.provider.current} /> : <span />}
                            </div>
                            <div className="instruction-action-col">
                              <button className="secondary-btn small" onClick={() => switchInstructionTemplate(item.id)} disabled={loading || isCurrent}>{t.instruction.enable}</button>
                              <button className="ghost-btn small" onClick={disableInstruction} disabled={loading || !isCurrent}>{i18n("禁用", "Disable")}</button>
                            </div>
                          </div>
                        );
                      })}

                      {savedPrompts.map((prompt) => {
                        const isCurrent = Boolean(state.instructionFile) && isInstructionFile(state.instructionFile, prompt.filename);
                        return (
                          <div className={cx("instruction-row", isCurrent && "selected")} key={prompt.id}>
                            <div className="instruction-icon custom"><FileCode2 size={22} /></div>
                            <div className="instruction-main">
                              <strong>{prompt.title}</strong>
                              <p>{i18n("自定义指令提示词", "Custom instruction prompt")}</p>
                              <code>./{prompt.filename}</code>
                            </div>
                            <div className="instruction-status-col">
                              {isCurrent ? <StatusPill active label={t.provider.current} /> : <span />}
                            </div>
                            <div className="instruction-action-col">
                              <button className="secondary-btn small" onClick={() => enableSavedPrompt(prompt.id)} disabled={loading || isCurrent}>{t.instruction.enable}</button>
                              <button className="ghost-btn small" onClick={disableInstruction} disabled={loading || !isCurrent}>{i18n("禁用", "Disable")}</button>
                              <button className="ghost-btn small" onClick={() => openEditPrompt(prompt)}>{t.provider.edit}</button>
                              <button className="danger-btn small" onClick={() => removeSavedPrompt(prompt.id)}><Trash2 size={14} /> {t.provider.remove}</button>
                            </div>
                          </div>
                        );
                      })}

                      {state.instructionFile && currentInstructionId === "custom" && !savedPrompts.some((p) => isInstructionFile(state.instructionFile, p.filename)) && (
                        <div className="instruction-row selected">
                          <div className="instruction-icon custom"><FileCode2 size={22} /></div>
                          <div className="instruction-main">
                            <strong>{i18n("当前自定义指令提示词", "Current custom prompt")}</strong>
                            <p>{i18n("当前 model_instructions_file 不是 Codex-X 内置模板。", "The current model_instructions_file is not a built-in Codex-X template.")}</p>
                            <code>{state.instructionFile}</code>
                          </div>
                          <div className="instruction-status-col"><StatusPill active label={t.provider.current} /></div>
                          <div className="instruction-action-col"><button className="ghost-btn small" onClick={disableInstruction} disabled={loading}>{i18n("禁用", "Disable")}</button></div>
                        </div>
                      )}
                    </div>
                  </>
                ) : (
                  <div className="prompt-form-page">
                    <div className="panel-head">
                      <div>
                        <p className="eyebrow">Prompt</p>
                        <h3>{editingPromptId ? (i18n("编辑提示词", "Edit prompt")) : (i18n("添加提示词", "Add prompt"))}</h3>
                      </div>
                      <button className="ghost-btn" onClick={() => setInstructionMode("list")}>{t.provider.cancel}</button>
                    </div>
                    <div className="form-grid prompt-form-grid">
                      <Field label={i18n("提示词名称", "Prompt name")}><input value={promptForm.title} onChange={(e) => setPromptForm({ ...promptForm, title: e.target.value, id: editingPromptId || providerId(e.target.value) })} /></Field>
                      <Field label={i18n("文件名", "Filename")}><input value={promptForm.filename} onChange={(e) => setPromptForm({ ...promptForm, filename: e.target.value })} placeholder="my-prompt.md" /></Field>
                      <label className="field prompt-content-field">
                        <span>{i18n("提示词内容", "Prompt content")}</span>
                        <textarea className="prompt-editor" value={promptForm.content} onChange={(e) => setPromptForm({ ...promptForm, content: e.target.value })} spellCheck={false} />
                      </label>
                    </div>
                    <div className="form-actions">
                      <button className="secondary-btn big" onClick={savePromptOnly} disabled={loading}>{i18n("保存", "Save")}</button>
                      <button className="primary-btn big" onClick={saveAndEnablePrompt} disabled={loading}><Zap size={18} /> {i18n("保存并启用", "Save & enable")}</button>
                    </div>
                  </div>
                )}
              </section>
            )}

            {tab === "toml" && (
              <section className="panel glass code-panel">
                <div className="panel-head">
                  <div>
                    <p className="eyebrow">~/.codex/config.toml</p>
                    <h3>{t.toml.title}</h3>
                    <p className="muted-desc">{t.toml.desc}</p>
                  </div>
                  <StatusPill active={state.configExists} label={state.configExists ? t.toml.loaded : t.dashboard.missing} />
                </div>
                <TomlPreview text={state.configText || t.toml.missingText} />
              </section>
            )}



            {tab === "about" && (
              <section className="about-page">
                <section className="panel glass about-card">
                  <div className="panel-head compact">
                    <div><p className="eyebrow">About</p><h3>{i18n("关于 Codex-X", "About Codex-X")}</h3></div>
                  </div>
                  <div className="about-kv">
                    <div><span>Codex-X {i18n("版本", "Version")}</span><strong>{aboutInfo?.appVersion || "0.2.0"}</strong></div>
                    <div><span>Codex {i18n("版本", "Version")}</span><strong>{aboutInfo?.codexVersion || (i18n("未检测到", "Not detected"))}</strong></div>
                    <div><span>CODEX_HOME</span><code>{aboutInfo?.codexDir || state.codexDir}</code></div>
                    <div><span>{i18n("项目地址", "Project")}</span><code>{aboutInfo?.projectUrl || `https://github.com/${FALLBACK_GITHUB_REPO}`}</code></div>
                  </div>
                  <div className="about-actions">
                    <button className="secondary-btn" onClick={() => openExternalUrl(aboutInfo?.projectUrl || `https://github.com/${FALLBACK_GITHUB_REPO}`)}><ExternalLink size={16} /> {i18n("打开项目主页", "Open project")}</button>
                    <button className="ghost-btn" onClick={() => openExternalUrl(`${aboutInfo?.projectUrl || `https://github.com/${FALLBACK_GITHUB_REPO}`}/issues`)}><ExternalLink size={16} /> {i18n("反馈问题", "Issues")}</button>
                  </div>
                </section>

                <section className="panel glass about-card">
                  <div className="panel-head compact">
                    <div><p className="eyebrow">GitHub Releases</p><h3>{i18n("更新检查", "Update check")}</h3></div>
                    <span className={cx("update-status-pill", releaseInfo.hasUpdate && "available")}>{releaseStatusLabel}</span>
                  </div>
                  <div className="about-kv">
                    <div><span>{i18n("状态", "Status")}</span><strong>{releaseStatusLabel}</strong></div>
                    <div><span>{i18n("最新版本", "Latest")}</span><strong>{releaseInfo.latestVersion || "-"}</strong></div>
                  </div>
                  <div className="about-actions">
                    <button className="primary-btn" onClick={() => void checkForUpdates()} disabled={releaseInfo.status === "checking"}><RefreshCw size={16} className={cx(releaseInfo.status === "checking" && "spin")} /> {i18n("检查更新", "Check updates")}</button>
                    <button className="secondary-btn" onClick={() => openExternalUrl(releaseInfo.htmlUrl)} disabled={!releaseInfo.htmlUrl}><Download size={16} /> {i18n("打开下载页", "Open releases")}</button>
                  </div>
                </section>
              </section>
            )}

            {tab === "settings" && (
              <section className="panel glass settings-panel">
                <div className="panel-head">
                  <div><p className="eyebrow">Settings</p><h3>{t.settings.title}</h3></div>
                </div>
                <div className="settings-list">
                  <div className="settings-row">
                    <div className="settings-icon"><Globe2 size={20} /></div>
                    <div className="settings-copy"><strong>{t.settings.language}</strong><p>{t.settings.languageDesc}</p></div>
                    <div className="segmented">
                      <button className={cx(lang === "zh" && "active")} onClick={() => setLang("zh")}>{t.settings.zh}</button>
                      <button className={cx(lang === "en" && "active")} onClick={() => setLang("en")}>{t.settings.en}</button>
                    </div>
                  </div>
                  <div className="settings-row">
                    <div className="settings-icon"><Sparkles size={20} /></div>
                    <div className="settings-copy"><strong>{t.settings.productName}</strong><p>{t.settings.productDesc}</p></div>
                    <StatusPill active label="Codex-X" />
                  </div>
                </div>
              </section>
            )}
          </>
        )}
      </section>
    </main>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode><App /></React.StrictMode>,
);
