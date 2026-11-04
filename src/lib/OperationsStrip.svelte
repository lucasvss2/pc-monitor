<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { MODULES, type Module, type Snapshot, type Operation, type AgentsData, type AgentSession, type SystemData, type NetworkData } from "./core";
  import { t } from "./strings";

  interface Clock {
    after(delay: number, callback: () => void): () => void;
  }
  let { snapshots, ready, clock = { after: (delay, callback) => {
    const timer = setTimeout(callback, delay);
    return () => clearTimeout(timer);
  } } }: { snapshots: Partial<Record<Module, Snapshot>>; ready: boolean; clock?: Clock } = $props();
  type Source = { module: Module; source: string; provider?: "codex" | "claude"; kind?: "movement" | "speed" | "transition"; message?: string; operation?: Operation };
  type Entry = { module: Module; operation: Operation; text: string; priority: number };
  let entries = $state<Record<string, Entry | undefined>>({});
  let announcement = $state("");
  let capacity = $state(3);
  let host: HTMLElement;
  let initialized = false;
  const seen = new Map<string, string>();
  const timers = new Map<string, () => void>();
  const pendingAnnouncements = new Map<string, string>();
  const networkReference = new Map<string, unknown>();
  const networkChanges = new Map<string, { operation: Operation; message: string }>();
  let networkId = 0;
  function networkSources(data?: NetworkData) {
    const messages = t.operations.network;
    const categories = [
      ["identity", data?.connectionRevision ?? 0, messages.changed, true],
      ["connection", data?.connected, data?.connected ? messages.connected : messages.disconnected, data?.connected],
      ["vpn", data?.vpn, data?.vpn ? messages.vpnOn : messages.vpnOff, !data?.vpn],
      ["quality", data?.quality, data?.quality && Object.hasOwn(messages, data.quality) ? messages[data.quality] : messages.qualityUnavailable, data?.quality === "good"],
      ["security", data?.security, data?.security && Object.hasOwn(messages, data.security) ? messages[data.security] : messages.securityUnavailable, data?.security === "noAlerts"],
    ] as const;
    return categories.map(([category, value, message, recovered]) => {
      const source = `network:${category}`;
      if (value !== undefined) {
        if (networkReference.has(source) && networkReference.get(source) !== value) {
          networkChanges.set(source, { operation: { id: ++networkId, origin: "automatic", state: recovered ? "recovered" : "attention" }, message });
        }
        networkReference.set(source, value);
      }
      return { module: "network" as const, source, ...networkChanges.get(source) };
    });
  }
  const agentReference = new Map<string, AgentSession["state"]>();
  const systemReference = new Map<string, boolean>();
  const transitionChanges = new Map<string, Source>();
  let transitionId = 0;
  let agentsInitialized = false;
  let systemInitialized = false;
  function agentSources(data: AgentsData): Source[] {
    const counts = { waiting: 0, done: 0, failed: 0, closed: 0 };
    if (!data.failed) {
      const current = new Map<string, AgentSession["state"]>();
      for (const session of [...data.sessions, ...data.closed]) {
        const identity = `${session.provider ?? "claude"}:${session.id}:${session.startedAt}`;
        current.set(identity, session.state);
        const previous = agentReference.get(identity);
        if (agentsInitialized && previous !== session.state && (previous !== undefined || session.state !== "closed") && session.state !== "working") counts[session.state]++;
      }
      agentsInitialized = true;
      agentReference.clear();
      current.forEach((state, identity) => agentReference.set(identity, state));
    }
    for (const state of ["failed", "waiting", "done", "closed"] as const) {
      const source = `agents:${state}`;
      if (counts[state]) transitionChanges.set(source, {
        module: "agents", source, kind: "transition", message: `${t.operations.agents[state]}: ${counts[state]}.`,
        operation: { id: ++transitionId, origin: "automatic", state: state === "failed" ? "failed" : state === "waiting" ? "attention" : "cancelled" },
      });
    }
    return [...transitionChanges.values()].filter(source => source.module === "agents");
  }
  function systemSources(data: SystemData): Source[] {
    const readings: [string, keyof typeof t.operations.system, boolean][] = [];
    if (data.cpu.temperature) readings.push(["temperature", "temperature", data.cpu.temperature.warning]);
    if (data.battery) readings.push(["battery", "battery", data.battery.warning]);
    for (const disk of data.disks) readings.push([`disk:${disk.mount}`, "disk", disk.warning]);
    const current = new Map<string, boolean>();
    const changes = new Map<string, { category: keyof typeof t.operations.system; warning: boolean; count: number }>();
    for (const [identity, category, warning] of readings) {
      current.set(identity, warning);
      const previous = systemReference.get(identity);
      if (systemInitialized && previous !== warning && (previous !== undefined || warning)) {
        const key = `${category}:${warning}`;
        changes.set(key, { category, warning, count: (changes.get(key)?.count ?? 0) + 1 });
      }
    }
    systemInitialized = true;
    systemReference.clear();
    current.forEach((warning, identity) => systemReference.set(identity, warning));
    for (const [key, { category, warning, count }] of changes) {
      const source = `system:${key}`;
      transitionChanges.set(source, { module: "system", source,
        message: `${t.operations.system[category][warning ? "warning" : "recovered"]}${count > 1 ? ` (${count})` : ""}`,
        operation: { id: ++transitionId, origin: "automatic", state: warning ? "attention" : "recovered" },
      });
    }
    return [...transitionChanges.values()].filter(source => source.module === "system");
  }
  let announcing = false;
  let disposed = false;

  async function announce() {
    if (announcing) return;
    announcing = true;
    // A new operation may have exactly the same result text as the previous one.
    announcement = "";
    await tick();
    if (!disposed) announcement = [...pendingAnnouncements.values()].join(" · ");
    pendingAnnouncements.clear();
    announcing = false;
  }
  onDestroy(() => { disposed = true; timers.forEach(cancel => cancel()); });

  // Only the closed operational vocabulary reaches the strip; source payload content is never rendered.
  function describe({ module, operation, provider, kind, message }: Source & { operation: Operation }): Entry | null {
    const speed = kind === "speed";
    const state = operation.state;
    if (!speed && state === "running" && operation.origin === "automatic") return null;
    if (!speed && state === "succeeded" && operation.origin !== "user") return null;
    const messages = speed ? { ...t.operations.messages, ...t.operations.speed } : kind === "movement" ? t.operations.movement : t.operations.messages;
    if (!Object.hasOwn(messages, state)) return null;
    const text = message ?? (state === "running" && operation.origin === "resume" ? t.operations.resume : messages[state]);
    const priority = state === "failed" || state === "unconfirmed" ? 0 : state === "paused" ? 1 : state === "attention" || state === "unavailable" ? 2 : state === "running" ? 3 : 4;
    return { module, operation, text: `${provider ? t.operations.providers[provider] : t.modules[module]} · ${text}`, priority };
  }

  $effect(() => {
    if (!ready) {
      untrack(() => {
        timers.forEach(cancel => cancel());
        timers.clear();
        entries = {};
        announcement = "";
        pendingAnnouncements.clear();
        seen.clear();
        agentsInitialized = false;
        systemInitialized = false;
        agentReference.clear();
        systemReference.clear();
        transitionChanges.clear();
        networkReference.clear();
        networkChanges.clear();
        initialized = false;
      });
      return;
    }
    const current = MODULES.flatMap<Source>(module => {
      const data = snapshots[module]?.data as { operation?: Operation; movement?: Operation; codex?: { operation?: Operation }; claude?: { operation?: Operation } } | undefined;
      if (module === "ai" && (data?.codex || data?.claude)) {
        return (["codex", "claude"] as const).map(provider => ({ module, source: `ai:${provider}`, provider, operation: data[provider]?.operation }));
      }
      if (module === "network" && data) {
        const network = snapshots.network?.data as NetworkData;
        return [...networkSources(network), { module, source: "network:speed", kind: "speed", operation: network.speedOperation }];
      }
      if (module === "agents" && data && "sessions" in data) return agentSources(data as unknown as AgentsData);
      if (module === "system" && data && "cpu" in data) return systemSources(data as unknown as SystemData);
      if (module === "tasks") {
        return [
          { module, source: `tasks:${snapshots.tasks?.contextGeneration ?? 0}:refresh`, operation: data?.operation },
          { module, source: `tasks:${snapshots.tasks?.contextGeneration ?? 0}:movement`, kind: "movement", operation: data?.movement },
        ];
      }
      return [{ module, source: module, provider: undefined, operation: data?.operation }];
    });
    untrack(() => {
      const activeTaskSources = new Set(current.filter(source => source.module === "tasks").map(source => source.source));
      for (const key of Object.keys(entries)) {
        if (key.startsWith("tasks:") && !activeTaskSources.has(key)) { timers.get(key)?.(); timers.delete(key); delete entries[key]; seen.delete(key); }
      }
      for (const { module, source, provider, kind, message, operation } of current) {
        if (!operation) {
          timers.get(source)?.();
          timers.delete(source);
          entries[source] = undefined;
          continue;
        }
        const key = `${operation.contextGeneration ?? 0}:${operation.id}:${operation.state}`;
        if (seen.get(source) === key) continue;
        seen.set(source, key);
        if (!initialized) continue;
        timers.get(source)?.();
        timers.delete(source);
        const entry = describe({ module, source, operation, provider, kind, message });
        entries[source] = entry ?? undefined;
        if (!entry) continue;
        if (operation.state !== "running") {
          if (operation.state !== "unconfirmed") timers.set(source, clock.after(4000, () => {
            entries[source] = undefined;
            timers.delete(source);
          }));
          if (kind === "transition" || operation.origin === "user" || ["failed", "recovered", "paused", "attention", "unavailable", "unconfirmed"].includes(operation.state)) {
            pendingAnnouncements.set(source, entry.text);
          }
        }
      }
      initialized = true;
      if (pendingAnnouncements.size) void announce();
    });
  });
  const ordered = $derived(MODULES.flatMap(module => {
    const sources = Object.values(entries).filter((entry): entry is Entry => entry?.module === module).sort((a, b) => a.priority - b.priority);
    return sources.length ? [{ text: sources.map(entry => entry.text).join("; "), priority: sources[0].priority }] : [];
  }).sort((a, b) => a.priority - b.priority));
  const visible = $derived(ordered.slice(0, capacity));
  onMount(() => {
    if (typeof ResizeObserver !== "function") return;
    const observer = new ResizeObserver(() => {
      // Reserve room for the overflow count; larger text naturally reduces the number of summaries.
      const size = parseFloat(getComputedStyle(host).fontSize) || 12;
      capacity = Math.max(1, Math.min(3, Math.floor((host.clientWidth - 9 * size) / (24 * size))));
    });
    observer.observe(host);
    return () => observer.disconnect();
  });
</script>

<div class="operations" aria-label="Faixa de bordo" bind:this={host}>
  <div class="summaries">
    {#each visible as entry}<span>{entry.text}</span>{:else}<span>{t.operations.rest}</span>{/each}
  </div>
  {#if ordered.length > visible.length}<span class="overflow">+{ordered.length - visible.length} Módulos</span>{/if}
</div>
<div class="sr-only" role="status" aria-live="polite" aria-atomic="true">{announcement}</div>

<style>
  .operations { display: flex; align-items: center; gap: 12px; flex: none; min-width: 0; min-height: 40px; margin: 6px -18px -10px; padding: 8px 18px; border-top: 1px solid var(--accent); background: var(--surface); color: var(--text); font: 12px monospace; }
  .summaries { display: flex; flex: 1; gap: 16px; min-width: 0; }
  .summaries span { flex: 0 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .overflow { flex: none; }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  @media (max-width: 650px) { .operations { margin: 6px -4px -4px; padding: 8px; } }
</style>
