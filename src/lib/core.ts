// Core to UI contract: push one snapshot per module; the UI renders and sends commands.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const MODULES = ["ai", "agents", "github", "trello", "network", "system"] as const;
export type Module = (typeof MODULES)[number];

/** Transient metadata only; never cached or used as a data timestamp. */
export interface Operation {
  id: number;
  origin: "user" | "automatic" | "resume";
  state: "running" | "succeeded" | "failed" | "recovered" | "paused" | "unavailable" | "cancelled" | "attention";
}

export interface Snapshot {
  module: Module;
  /** Native publications are ordered by runtime/revision, never the civil clock. */
  runtimeId?: string;
  revision?: number;
  contextGeneration?: number;
  /** Last update in epoch milliseconds; null until data exists. */
  updatedAt: number | null;
  data: unknown;
}

/** Bytes. */
export interface Memory {
  used: number;
  total: number;
}

/** A mounted disk in bytes; free is what df reports as available. */
export interface Disk {
  mount: string;
  used: number;
  total: number;
  free: number;
  warning: boolean;
}

/** System snapshot data, including warning decisions made by the core. */
export interface SystemData {
  cpu: {
    /** Percentage; null until two samples exist. */
    usage: number | null;
    /** One sample every two seconds, oldest to newest. */
    history: number[];
    temperature: { celsius: number; warning: boolean } | null;
  };
  ram: Memory;
  swap: Memory | null;
  disks: Disk[];
  battery: {
    percentage: number;
    state: "charging" | "discharging" | "full" | "empty" | "pluggedIn" | "unknown";
    secondsRemaining: number | null;
    warning: boolean;
  } | null;
}

export type Theme = "dark" | "light" | "system";
export type SystemTheme = "dark" | "light";
/** Persisted visual preferences; the system reduced-motion preference prevails. */
export interface Appearance { theme: Theme; reduceMotion: boolean }

export interface GeneralSettings {
  version: string;
  autostart: boolean;
  versionCheck: boolean;
  /** Newer release found by the daily check; never installed by the app. */
  release: { version: string; url: string } | null;
}

export interface InitialState {
  runtimeId?: string;
  /** Capabilities not implemented by the current native adapter. */
  unavailableModules?: Module[];
  panelVisible?: boolean;
  /** First opening: Settings shows the welcome. */
  welcome?: boolean;
  appearance?: Appearance;
  /** Desktop color scheme from the core; null when unknown. */
  systemTheme?: SystemTheme | null;
  /** Empty string targets Agents without identifying a particular session. */
  agentTarget?: string | null;
  /** Last module chosen in the navigation, or Agents for a notification target. */
  lastAnchor: Module;
  snapshots: Snapshot[];
}

export interface AgentNotificationPreferences { waiting: boolean; done: boolean; failed: boolean }

export interface Core {
  listenPanelVisibility?(cb: (visible: boolean) => void): Promise<() => void>;
  listenAgentTarget?(cb: (session: string) => void): Promise<() => void>;
  setAgentHooks?(enabled: boolean): Promise<void>;
  setAgentNotifications?(preferences: AgentNotificationPreferences): void;
  initialState(): Promise<InitialState>;
  listenSnapshots(cb: (s: Snapshot) => void): Promise<() => void>;
  /** Modules intersecting the visible panel area, in MODULES order; empty in Settings. */
  reportVisibleModules(modules: Module[]): void;
  /** The module chosen in the navigation, restored on the next opening. */
  chooseAnchor(module: Module): void;
  runSpeedTest?(): void;
  setAutomaticSpeed?(enabled: boolean): void;
  refreshAi?(): void;
  /** Rejects with a ClaudeError code; settings.json is changed only here. */
  connectClaude?(): Promise<void>;
  disconnectClaude?(): Promise<void>;
  /** Opt-in to the undocumented oauth/usage endpoint; rejects with a ClaudeError code. */
  setClaudeUsage?(enabled: boolean): Promise<void>;
  refreshGithub?(): void;
  saveGithubToken?(token: string): Promise<string>;
  refreshTrello?(): void;
  /** Resolves with the account login; rejects with a credential error code. */
  connectTrello?(key: string, token: string): Promise<string>;
  selectTrelloBoard?(board: string): Promise<void>;
  disconnectTrello?(): Promise<void>;
  openTrelloAdmin?(): void;
  openTrelloAuthorization?(key: string): void;
  openTrelloCard?(url: string): void;
  /** Rejects when Trello refuses or cannot be reached. */
  moveTrelloCard?(card: string, list: string): Promise<void>;
  closePanel(): void;
  listenSystemTheme?(cb: (theme: SystemTheme) => void): Promise<() => void>;
  /** The welcome was shown; it never opens again. */
  completeWelcome?(): void;
  listenGeneralSettings?(cb: (settings: GeneralSettings) => void): Promise<() => void>;
  generalSettings?(): Promise<GeneralSettings>;
  /** Rejects with "unwritable" when the autostart entry cannot change. */
  setAutostart?(enabled: boolean): Promise<GeneralSettings>;
  setVersionCheck?(enabled: boolean): Promise<GeneralSettings>;
  openRelease?(): void;
  openLogs?(): Promise<void>;
  setAppearance?(appearance: Appearance): Promise<void>;
}

export const tauriCore: Core = {
  listenAgentTarget: (cb) => listen<string>("agent-target", (e) => cb(e.payload)),
  setAgentHooks: (enabled) => invoke<void>("set_agent_hooks", { enabled }),
  setAgentNotifications: (preferences) => void invoke("set_agent_notifications", { preferences }),
  listenPanelVisibility: (cb) => listen<boolean>("panel-visibility", (e) => cb(e.payload)),
  initialState: () => invoke<InitialState>("initial_state"),
  async listenSnapshots(cb) {
    const stops = await Promise.all(MODULES.map((m) => listen<Snapshot>(`snapshot:${m}`, (e) => cb(e.payload))));
    return () => stops.forEach((stop) => stop());
  },
  reportVisibleModules: (modules) => void invoke("report_visible_modules", { modules }),
  chooseAnchor: (module) => void invoke("choose_anchor", { module }),
  runSpeedTest: () => void invoke("run_speed_test"),
  setAutomaticSpeed: (enabled) => void invoke("set_automatic_speed", { enabled }),
  saveGithubToken: (token) => invoke<string>("save_github_token", { token }),
  refreshAi: () => void invoke("refresh_ai"),
  connectClaude: () => invoke<void>("connect_claude"),
  disconnectClaude: () => invoke<void>("disconnect_claude"),
  setClaudeUsage: (enabled) => invoke<void>("set_claude_usage", { enabled }),
  refreshGithub: () => void invoke("refresh_github"),
  refreshTrello: () => void invoke("refresh_trello"),
  connectTrello: (key, token) => invoke<string>("connect_trello", { key, token }),
  selectTrelloBoard: (board) => invoke<void>("select_trello_board", { board }),
  disconnectTrello: () => invoke<void>("disconnect_trello"),
  openTrelloAdmin: () => void invoke("open_trello_admin"),
  openTrelloAuthorization: (key) => void invoke("open_trello_authorization", { key }),
  openTrelloCard: (url) => void invoke("open_trello_card", { url }),
  moveTrelloCard: (card, list) => invoke<void>("move_trello_card", { card, list }),
  closePanel: () => void invoke("close_panel"),
  listenSystemTheme: (cb) => listen<SystemTheme>("system-theme", (e) => cb(e.payload)),
  completeWelcome: () => void invoke("complete_welcome"),
  listenGeneralSettings: (cb) => listen<GeneralSettings>("general-settings", (e) => cb(e.payload)),
  generalSettings: () => invoke<GeneralSettings>("general_settings"),
  setAutostart: (enabled) => invoke<GeneralSettings>("set_autostart", { enabled }),
  setVersionCheck: (enabled) => invoke<GeneralSettings>("set_version_check", { enabled }),
  openRelease: () => void invoke("open_release"),
  openLogs: () => invoke<void>("open_logs"),
  setAppearance: (appearance) => invoke<void>("set_appearance", { appearance }),
};

export interface NetworkMeasurements { latency: number | null; jitter: number | null; loss: number | null }
export interface NetworkTraffic { download: number; upload: number }
export interface SpeedOperation extends Operation {
  reason?: "connectionChanged" | "disconnected" | "metered" | "unavailable" | "suspended";
}

export interface NetworkData {
  connectionRevision?: number;
  speedOperation?: SpeedOperation;
  processTraffic: { apps: { name: string; download: number; upload: number }[]; others: NetworkTraffic } | null;
  speed: { download: number; upload: number; measuredAt: number; networkId: string } | null;
  speedAvailable: boolean;
  speedRunning: boolean;
  speedFailed: boolean;
  automaticSpeed: boolean;
  connected: boolean;
  wired: boolean;
  vpn: boolean;
  quality: "good" | "regular" | "poor" | null;
  security: "noAlerts" | "warning" | "risk" | null;
  reasons: string[];
  gateway: NetworkMeasurements;
  internet: NetworkMeasurements;
  traffic: NetworkTraffic | null;
  history: NetworkTraffic[];
}

/** One usage window; the core nulls the percent once the window restarts. */
export interface AiWindow {
  usedPercent: number | null;
  durationMins: number | null;
  /** Epoch milliseconds. */
  resetsAt: number | null;
  /** Restarted after the reading: never shown as 0%. */
  reset: boolean;
}
export interface AiReading {
  source: "appServer" | "rollout" | "statusLine" | "oauthUsage";
  readAt: number;
  buckets: { id: string; name: string | null; primary: AiWindow | null; secondary: AiWindow | null }[];
}
export interface AiData {
  refreshing: boolean;
  paused?: boolean;
  codex: {
    operation?: Operation;
    state: "missing" | "empty" | "ready";
    /** First of at least three consecutive failed queries. */
    failureSince?: number | null;
    /** Codex planType, e.g. plus or pro. */
    plan: string | null;
    reading: AiReading | null;
  };
  claude?: {
    operation?: Operation;
    state: "missing" | "empty" | "ready";
    /** Claude subscriptionType, e.g. pro or max. */
    plan: string | null;
    /** replaced: another tool changed statusLine after connecting. */
    statusLine: "off" | "connected" | "replaced";
    /** oauth/usage opt-in; expired: token expired or refused, back to the status line. */
    usage: "off" | "on" | "expired";
    /** Opted in, but the local access token could not be read. */
    usageUnavailable?: boolean;
    /** First of at least three consecutive failed oauth/usage reads. */
    failureSince?: number | null;
    /** oauth/usage waits for connectivity or its turn after login/reconnection. */
    paused?: boolean;
    reading: AiReading | null;
  };
}

export interface GithubData {
  /** Credential metadata only; source is gh or token. */
  credential?: { source: string; login: string };
  paused?: boolean;
  lastSuccessAt?: number | null;
  failureSince?: number | null;
  state: string;
  refreshing: boolean;
  calendar: { restrictedContributionsCount: number; contributionCalendar: {totalContributions:number; weeks: {contributionDays: {date:string; contributionCount:number; color:string}[]}[]} } | null;
  featured: {name:string; commits:number; total:number; title:string; when:number} | null;
  /** Contribution streak in days, ending today or yesterday; null without a calendar. */
  streak?: number | null;
}

export interface TrelloCard {
  id: string;
  /** Empty when restored from the disk cache, which never stores titles. */
  name: string;
  url: string;
  /** Trello color names, e.g. green or sky_light. */
  labels: string[];
  due: number | null;
  dueComplete: boolean;
  overdue: boolean;
}
export interface TrelloData {
  operation?: Operation;
  movement?: Operation;
  state: "missingCredential" | "chooseBoard" | "ready" | "stale" | "error" | "invalidCredential";
  refreshing: boolean;
  paused?: boolean;
  lastSuccessAt?: number | null;
  failureSince?: number | null;
  account: { login: string } | null;
  boards: { id: string; name: string }[];
  board: { id: string; name: string } | null;
  lists: { id: string; name: string; cards: TrelloCard[] }[] | null;
  /** Key typed before generating the token; kept in core memory only. */
  pendingKey?: string;
}

export interface AgentSession {
  provider?: "claude" | "codex";
  branch?: string | null;
  updatedAt?: number | null;
  id: string;
  project: string;
  name: string | null;
  state: "working" | "waiting" | "done" | "failed" | "closed";
  waitingFor: string | null;
  startedAt: number;
  closedAt: number | null;
  lastState?: "working" | "waiting" | "done" | "failed" | null;
}
export interface AgentsData {
  discoveryPartial?: boolean;
  notificationsAvailable?: boolean;
  hooksEnabled?: boolean;
  notificationPreferences?: AgentNotificationPreferences;
  codexWaitingUnavailable?: boolean;
  sessions: AgentSession[];
  closed: AgentSession[];
  accentDot: boolean;
  failed: boolean;
  observedAt: number;
  /** Last successful read; after a failure the sessions shown are this old. */
  lastSuccessAt?: number | null;
}
