import type { AiData, GithubData, Module, Snapshot, TasksData } from "../core";

export type ModuleStatus = "detected" | "needsSetup" | "notNeeded" | "checking";

/** Claude Code is installed but nothing feeds its limits yet. */
export function claudeNeedsConnection(data: AiData | undefined): boolean {
  const claude = data?.claude;
  return !!claude && claude.state !== "missing" && claude.statusLine !== "connected" && claude.usage !== "on";
}

/** Welcome status of a module, from its latest snapshot. */
export function moduleStatus(module: Module, snapshot: Snapshot | undefined): ModuleStatus {
  if (module === "agents" || module === "network" || module === "system") return "notNeeded";
  if (!snapshot?.data) return "checking";
  if (module === "ai") {
    const data = snapshot.data as AiData;
    const noProvider = data.codex.state === "missing" && (!data.claude || data.claude.state === "missing");
    return claudeNeedsConnection(data) || noProvider ? "needsSetup" : "detected";
  }
  if (module === "github") {
    const data = snapshot.data as GithubData;
    if (data.credential) return "detected";
    return data.state === "missingGh" || data.state === "invalidCredential" ? "needsSetup" : "detected";
  }
  const data = snapshot.data as TasksData;
  if (data.initializing) return "checking";
  return ["missingCredential", "chooseBoard", "invalidCredential", "cleanupPending", "settingsUnavailable"].includes(data.state) ? "needsSetup" : "detected";
}
