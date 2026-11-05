<script lang="ts">
  import type { AgentsData, AiData, GithubData, Module, NetworkData, Snapshot, SystemData, TasksData } from "./core";
  import { installed, remainingPercent, usageWindows } from "./ai/profile";
  import { interpolate, t } from "./strings";

  let { snapshots, unavailable, now, expand }:
    { snapshots: Partial<Record<Module, Snapshot>>; unavailable: Module[]; now: number; expand: (module: Module) => void } = $props();
  const ai = $derived(snapshots.ai?.data as AiData | undefined);
  const agents = $derived(snapshots.agents?.data as AgentsData | undefined);
  const tasks = $derived(snapshots.tasks?.data as TasksData | undefined);
  const github = $derived(snapshots.github?.data as GithubData | undefined);
  const network = $derived(snapshots.network?.data as NetworkData | undefined);
  const system = $derived(snapshots.system?.data as SystemData | undefined);
  const providers = $derived([
    { name: t.ai.codex, data: ai?.codex, stale: ai?.paused || ai?.codex.failureSince != null },
    { name: t.ai.claude, data: ai?.claude, stale: ai?.claude?.paused || ai?.claude?.failureSince != null },
  ].filter(provider => installed(provider.data)));
  const sessions = $derived(agents?.sessions ?? []);
  const number = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 });
  const count = (state: "working" | "done" | "failed" | "waiting") => sessions.filter(session => session.state === state).length;
  const taskProvider = $derived(tasks?.provider === "jira" ? "Jira" : "Trello");
  const taskBoard = $derived(tasks && "board" in tasks ? tasks.board : null);
  const taskCount = $derived(tasks && "issues" in tasks && tasks.state === "ready" ? tasks.issues.length
    : tasks && "lists" in tasks && tasks.lists ? tasks.lists.reduce((sum, list) => sum + list.cards.length, 0) : null);
  const taskPaused = $derived(tasks && "paused" in tasks && (tasks.paused || tasks.state === "stale" || ("requiresRead" in tasks && tasks.requiresRead)));
  const traffic = (bytes: number) => `${number.format(bytes / 1024)} KB/s`;
</script>

<div class="compact-body" aria-label={t.header.compactPanel}>
  <section class="quotas" data-compact-module="ai" aria-label={t.modules.ai}>
    <h2>{t.modules.ai}</h2>
    {#if unavailable.includes("ai")}
      <p>{t.header.unavailable}</p>
    {:else if providers.length === 0}
      <p>{!ai || ai.codex.installed === null || ai.claude?.installed === null ? t.ai.checkingTools : t.ai.noTools}</p>
    {:else}
      <div class="provider-grid">
        {#each providers as provider (provider.name)}
          {@const { five, week } = usageWindows(provider.data?.reading ?? null)}
          {@const main = five ?? week}
          {@const remaining = remainingPercent(main, now)}
          {@const weekly = remainingPercent(week, now)}
          <button class="quota" onclick={() => expand("ai")}>
            <span>{provider.name}</span>
            <strong class="num">{remaining === null ? "—" : `${remaining}%`}</strong>
            {#if remaining !== null}<span class="bar" aria-hidden="true"><i style:width={`${remaining}%`}></i></span>{/if}
            <small>{main === null || remaining === null ? t.ai.noReading : main === five ? t.ai.fiveHourRemaining : t.ai.weeklyLabel}</small>
            {#if five && weekly !== null}<small>{t.ai.weeklyLabel} · {weekly}%</small>{/if}
            {#if provider.stale}<small>{t.header.oldReading}</small>{/if}
          </button>
        {/each}
      </div>
    {/if}
  </section>

  <section data-compact-module="agents" aria-label={t.modules.agents}>
    <button class="summary agents" onclick={() => expand("agents")}>
      <strong>{t.modules.agents}</strong>
      {#if agents?.failed || unavailable.includes("agents")}<small>{t.agents.failure}</small>
      {:else if !agents || agents.lastSuccessAt == null && sessions.length === 0}<small>{t.agents.noReading}</small>
      {:else}
        <span class="agent-counts">
          <span><b>{count("working")}</b> {t.agents.states.working}</span>
          <span><b>{count("done")}</b> {t.agents.states.done}</span>
          <span><b>{count("failed")}</b> {t.agents.states.failed}</span>
        </span>
        {#if count("waiting") > 0}<small class="attention">{interpolate(t.agents.waitingCount, { count: String(count("waiting")) })}</small>
        {:else if agents.codexWaitingUnavailable}<small>{t.agents.codexWaitingUnavailable}</small>
        {:else}<small>{t.agents.noneWaiting}</small>{/if}
      {/if}
    </button>
  </section>

  <section data-compact-module="tasks" aria-label={t.modules.tasks}>
    <button class="summary tasks" onclick={() => expand("tasks")}>
      <strong>{t.modules.tasks}{#if tasks} <small>{taskProvider}</small>{/if}</strong>
      {#if taskBoard && taskCount !== null}
        <span>{taskBoard.name} · {interpolate(t.header.items, { count: String(taskCount) })}</span>
        {#if taskPaused}<small>{t.header.oldReading}</small>{/if}
      {:else}<small>{t.header.noData}</small>{/if}
    </button>
  </section>

  <div class="support">
    <section data-compact-module="github" aria-label={t.modules.github}>
      <button class="summary github" onclick={() => expand("github")}>
        <strong>{t.modules.github}</strong>
        {#if github?.calendar}<span class="num">{number.format(github.calendar.contributionCalendar.totalContributions)}</span><small>{t.header.contributions}</small>
          {#if github.paused || github.failureSince != null}<small>{t.header.oldReading}</small>{/if}
        {:else}<small>{t.header.noData}</small>{/if}
      </button>
    </section>
    <section data-compact-module="network" aria-label={t.modules.network}>
      <button class="summary network" onclick={() => expand("network")}>
        <strong>{t.modules.network}</strong>
        {#if network?.connected}
          <span>{network.internet.latency === null ? t.network.connected : `${number.format(network.internet.latency)} ms`}</span>
          {#if network.traffic}<small>↓ {traffic(network.traffic.download)} · ↑ {traffic(network.traffic.upload)}</small>{/if}
        {:else}<small>{network && !network.connectionUnavailable ? t.network.disconnected : t.header.noData}</small>{/if}
      </button>
    </section>
    <section data-compact-module="system" aria-label={t.modules.system}>
      <button class="summary system" onclick={() => expand("system")}>
        <strong>{t.modules.system}</strong>
        {#if system}
          {#if system.cpu.usage !== null}<span class="num">CPU {Math.round(system.cpu.usage)}%</span>{/if}
          {#if system.ram.total > 0}<small>RAM {Math.round(system.ram.used / system.ram.total * 100)}%</small>{/if}
          {#if now - (snapshots.system?.updatedAt ?? now) > 10_000}<small>{t.header.oldReading}</small>{/if}
        {:else}<small>{t.header.noData}</small>{/if}
      </button>
    </section>
  </div>
</div>

<style>
  .compact-body { display: grid; gap: 8px; padding: 3px 3px 8px; }
  h2 { font-size: 1em; letter-spacing: .12em; text-transform: uppercase; margin-bottom: 7px; }
  .quotas { padding: 10px; background: var(--module-bg); border-top: 3px solid var(--band-lavender); }
  .provider-grid, .support { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 100px), 1fr)); gap: 6px; }
  .support { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  button { width: 100%; height: 100%; min-width: 0; padding: 8px; text-align: left; overflow-wrap: anywhere; }
  button:hover, button:focus-visible { border-color: var(--accent); background: var(--surface-2); }
  button:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .quota { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 4px; align-items: center; background: var(--surface); border: 1px solid var(--control-line); }
  .quota strong { font-size: 1.65em; }
  .quota small, .bar { grid-column: 1 / -1; }
  .bar { height: 4px; background: var(--line); }
  .bar i { height: 100%; display: block; background: var(--accent); }
  .quota + .quota .bar i { background: var(--band-lavender-text); }
  .summary { display: grid; align-content: start; gap: 4px; background: var(--module-bg); border: 1px solid var(--control-line); border-top: 3px solid var(--band-cyan); }
  .summary.tasks { border-top-color: var(--band-amber); }
  .summary.github { border-top-color: var(--band-steel); }
  .summary.network { border-top-color: var(--band-blue); }
  .summary.system { border-top-color: var(--band-lavender); }
  .agent-counts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 6px; }
  .agent-counts b { display: block; font-size: 1.35em; }
  .attention { color: var(--warn); }
  small, .quotas p { color: var(--muted); }
</style>
