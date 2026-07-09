export type ProviderMode = "list" | "form" | "official";
export type InstructionMode = "list" | "form";
export type Tab = "dashboard" | "provider" | "sessions" | "instruction" | "toml" | "settings" | "about";

export type InstructionTemplate = {
  id: string;
  filename: string;
  title: string;
  subtitle: string;
  badge: string;
};

export type ProviderSummary = {
  id: string;
  name?: string;
  baseUrl?: string;
  wireApi?: string;
  requiresOpenaiAuth?: boolean;
  isCurrent: boolean;
};

export type SavedProvider = {
  id: string;
  providerName: string;
  baseUrl: string;
  model: string;
  apiKey?: string;
  wireApi: string;
  requiresOpenaiAuth: boolean;
};

export type SavedPrompt = {
  id: string;
  title: string;
  filename: string;
  content: string;
};

export type BackupEntry = {
  id: string;
  action: string;
  createdAt: string;
  path: string;
  hadConfig: boolean;
  hadAuth: boolean;
};

export type CodexState = {
  codexDir: string;
  configPath: string;
  authPath: string;
  configExists: boolean;
  authExists: boolean;
  officialAuthAvailable: boolean;
  model?: string;
  modelProvider?: string;
  instructionFile?: string;
  instructionEnabled: boolean;
  providers: ProviderSummary[];
  configText: string;
  authPreview?: unknown;
  authText: string;
  lastBackup?: BackupEntry;
};

export type ActionResult = {
  ok: boolean;
  message: string;
  backupId?: string;
  state: CodexState;
};

export type ImportResult = {
  imported: number;
  skipped: number;
  warnings: string[];
  providers: SavedProvider[];
};

export type OfficialAuthCandidate = {
  authJson: string;
  model?: string;
  source: string;
};

export type AboutInfo = {
  appVersion: string;
  codexVersion?: string;
  codexDir: string;
  projectUrl: string;
  githubRepo: string;
};

export type ReleaseInfo = {
  status: "idle" | "checking" | "ok" | "error";
  latestVersion?: string;
  htmlUrl?: string;
  assetName?: string;
  body?: string;
  message?: string;
  hasUpdate?: boolean;
};


export type SessionSyncStatus = {
  codexDir: string;
  targetProvider: string;
  rolloutFiles: number;
  sessionMetaCount: number;
  mismatchedRollouts: number;
  mismatchedSessionMeta: number;
  sqliteDbs: number;
  sqliteThreads: number;
  mismatchedThreads: number;
  needsSync: boolean;
  backupDir?: string | null;
  warnings: string[];
  sessions: SessionPreview[];
};

export type SessionPreview = {
  id: string;
  title: string;
  modelProvider?: string | null;
  model?: string | null;
  cwd?: string | null;
  rolloutPath?: string | null;
  updatedAtMs?: number | null;
  archived: boolean;
  hasUserEvent: boolean;
  needsSync: boolean;
};

export type SessionSyncResult = {
  status: SessionSyncStatus;
  updatedRollouts: number;
  updatedThreads: number;
  backupDir: string;
};
