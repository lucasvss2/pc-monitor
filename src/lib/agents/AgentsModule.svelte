<script lang="ts">
  import { tick, untrack } from "svelte";
  import type { AgentsData, AgentSession } from "../core";
  import { interpolate, t } from "../strings";
  import AbsentArea, { type Absence } from "../AbsentArea.svelte";

  /** `notified` is the session a notification pointed to; it is not the last anchor. */
  let { data, notified = null, notificationRequest = 0, configure = () => {} }:
    { data: AgentsData | null; notified?: string | null; notificationRequest?: number; configure?: () => void } = $props();

  /** Same allowlist as the core notifications: free-form waiting detail can carry user content. */
  const TOOLS = new Set(["Bash", "Read", "Write", "Edit", "Glob", "Grep", "WebFetch", "WebSearch", "Agent"]);
  const REASONS: Record<string, string> = {
    "permission prompt": t.agents.permission, "input needed": t.agents.input,
    "sandbox request": t.agents.sandbox, "dialog open": t.agents.dialog,
  };
  /** The five session states, in the legend's order; closed sessions count while retained. */
  const states = ["working", "waiting", "done", "failed", "closed"] as const;
  const colors: Record<AgentSession["state"], string> = {
    working: "var(--accent)", waiting: "var(--warn)", done: "var(--ok)",
    failed: "var(--bad)", closed: "var(--band-lavender-text)",
  };
  const time = new Intl.DateTimeFormat("pt-BR", { hour: "2-digit", minute: "2-digit" });

  let list = $state<HTMLElement>();

  let dialog: HTMLDialogElement;
  let detailOpen = $state(false);
  let restoringFocus = false;
  let opener: HTMLElement | null = null;
  let ring = $state<HTMLButtonElement>();
  let selected = $state<string | null>(null);
  const shown = $derived(data ? [...data.sessions, ...data.closed] : []);
  const count = (state: AgentSession["state"]) => shown.filter((s) => s.state === state).length;
  const waiting = $derived(count("waiting"));
  /** Ring segments over every retained session; no segment when there is none. */
  const segments = $derived.by(() => {
    let offset = 0;
    return states.map((state) => {
      const share = shown.length ? count(state) / shown.length * 100 : 0;
      const segment = { state, length: Math.max(0, share - 2), offset };
      offset += share;
      return segment;
    });
  });

  async function open(id: string | null = selected) {
    selected = id;
    if (!dialog.open) {
      opener = document.activeElement instanceof HTMLElement && document.activeElement !== document.body ? document.activeElement : ring ?? null;
      dialog.showModal();
      detailOpen = true;
    }
    await tick();
    Array.from(list?.children ?? []).find((row) => (row as HTMLElement).dataset.sessionId === id)
      ?.scrollIntoView?.({ block: "nearest" });
  }
  function close() {
    restoringFocus = true;
    dialog.close();
    detailOpen = false;
    opener?.focus({ preventScroll: true });
    restoringFocus = false;
  }
  function keyboard(event: KeyboardEvent) {
    if (event.key === "Escape" && dialog?.open) {
      event.preventDefault();
      event.stopPropagation();
      close();
    }
  }
  $effect(() => {
    notificationRequest;
    const id = notified;
    if (id) untrack(() => void open(id));
  });
  const targetAvailable = $derived(!!notified && shown.some((session) => session.id === notified));
  $effect(() => {
    const id = notified;
    if (!detailOpen || !targetAvailable) return;
    void tick().then(() => {
      Array.from(list?.children ?? []).find((row) => (row as HTMLElement).dataset.sessionId === id)
        ?.scrollIntoView?.({ block: "nearest" });
    });
  });
  const providerName = (session: AgentSession) => session.provider === "codex" ? t.agents.codex : t.agents.provider;

  function elapsed(session: AgentSession, observedAt: number) {
    const seconds = Math.max(0, Math.floor(((session.closedAt ?? observedAt) - session.startedAt) / 1000));
    if (seconds < 60) return `${seconds} s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)} min ${seconds % 60} s`;
    return `${Math.floor(seconds / 3600)} h ${String(Math.floor((seconds % 3600) / 60)).padStart(2, "0")} min`;
  }

  function reason(session: AgentSession) {
    if (session.state === "waiting") {
      const detail = session.waitingFor ?? "";
      return TOOLS.has(detail) ? interpolate(t.agentNotifications.approval, { tool: detail }) : REASONS[detail] ?? t.agents.waitingDetail;
    }
    if (session.state === "closed" && (session.lastState === "done" || session.lastState === "failed"))
      return interpolate(t.agents.lastState, { state: t.agents.states[session.lastState] });
    return null;
  }

  /** Without any reading, the ring, legend and notice give way to one area saying why. */
  const absence = $derived.by((): Absence | null => {
    if (!data) return { text: t.agents.noReading, tone: "neutral" };
    if (data.failed && data.lastSuccessAt == null) return { text: t.agents.failure, tone: "warn" };
    return null;
  });
  /** Sessions from before a failed read: dimmed under the failure line. */
  const old = $derived(!!data?.failed && !absence);

  const caption = $derived(!data ? t.agents.noReading : shown.length === 0 ? t.agents.empty
    : shown.length === 1 ? t.agents.oneSession : interpolate(t.agents.sessions, { count: String(shown.length) }));
</script>

<svelte:window onkeydowncapture={keyboard} />

<div class="agents" onfocusin={(event) => { if (restoringFocus) event.stopPropagation(); }}>
  {#if absence}
    <AbsentArea {absence} />
  {:else if data?.discoveryPartial && data.lastSuccessAt == null && shown.length === 0}
    <div class="waiting calm" role="status">
      <span>{data.hooksEnabled ? t.agents.waitingForHookEvents : t.agents.noAutomaticDiscovery}</span>
      {#if !data.hooksEnabled}<button class="see" onclick={configure}>{t.absent.configure}</button>{/if}
    </div>
  {:else}
  {#if data?.discoveryPartial}
    <small class="line" role="status">{t.agents.partialDiscovery}</small>
  {/if}
  {#if data?.failed}
    <small role="status" class="line alert"><span aria-hidden="true">{"⚠ "}</span>{[t.agents.failure, data.lastSuccessAt != null && interpolate(t.agents.lastRead, { time: time.format(data.lastSuccessAt) })].filter(Boolean).join(" · ")}</small>
  {/if}
  <!-- The notice box says it when it is free; a waiting session takes it first. -->
  {#if data?.codexWaitingUnavailable && waiting > 0}<small class="line">{t.agents.codexWaitingUnavailable}</small>{/if}

  <div class="readings" data-stale={old}>
  <div class="summary">
    <div class="agent-instrument">
      <button class="session-dial" bind:this={ring} aria-label={t.agents.list} onclick={() => open()}>
        <svg viewBox="0 0 120 120" aria-hidden="true">
          <circle class="orbit" cx="60" cy="60" r="55" />
          {#if shown.length === 0}
            <circle class="base" cx="60" cy="60" r="47" />
          {:else}
            {#each segments as segment (segment.state)}
              <circle cx="60" cy="60" r="47" fill="none" stroke={colors[segment.state]} stroke-width="8" pathLength="100"
                stroke-dasharray="{segment.length} 100" stroke-dashoffset={-segment.offset} transform="rotate(-90 60 60)" />
            {/each}
          {/if}
        </svg>
        <span class="reading num">{count("working")}</span>
      </button>
      <h3>{t.agents.states.working}</h3>
      <small>{caption}</small>
    </div>
    <div class="legend-list">
      {#each states as state (state)}
        <div class="legend-item">
          <button class="legend" style:--state-color={colors[state]} aria-describedby={"agent-legend-" + state}
            onclick={() => open(shown.find((s) => s.state === state)?.id ?? null)}>
            <i aria-hidden="true"></i><span>{t.agents.states[state]}</span> <b class="num">{count(state)}</b>
          </button>
          <span class="session-tip" id={"agent-legend-" + state}>{shown.filter((s) => s.state === state).map((s) => `${s.project} · ${s.name ?? t.agents.unnamed}`).join(" / ") || "—"}</span>
        </div>
      {/each}
    </div>
  </div>
  {#if waiting > 0}
    <button class="waiting" onclick={() => open(shown.find((s) => s.state === "waiting")?.id ?? null)}>
      <span><span aria-hidden="true">{"⚠ "}</span>{interpolate(t.agents.waitingCount, { count: String(waiting) })}</span>
      <span class="see">{t.agents.seeSessions}</span>
    </button>
  {:else if data?.codexWaitingUnavailable}
    <!-- Without instant detection Codex cannot report waiting: the box says so instead of claiming none. -->
    <div class="waiting calm">
      <span>{t.agents.codexWaitingUnavailable}{" · "}<button class="link" onclick={configure}>{t.absent.configure}</button></span>
      <button class="see" onclick={() => open()}>{t.agents.seeSessions}</button>
    </div>
  {:else}
    <!-- Keeps the notice space: an empty card bottom read as missing content. After a failed read it
         speaks of the last good one only, since the current state is unknown. -->
    <button class="waiting calm" onclick={() => open()}>
      <span><span aria-hidden="true">{"✓ "}</span>{old && data?.lastSuccessAt != null
        ? interpolate(t.agents.noneWaitingAt, { time: time.format(data.lastSuccessAt) }) : t.agents.noneWaiting}</span>
      <span class="see">{t.agents.seeSessions}</span>
    </button>
  {/if}
  </div>
  {/if}

  <dialog bind:this={dialog} aria-label={t.agents.list} oncancel={(event) => { event.preventDefault(); close(); }}>
    <header><h2>{t.agents.list}</h2><button class="detail-button" onclick={close}>{t.agents.close} <span aria-hidden="true">×</span></button></header>
    {#if notified && !shown.some((s) => s.id === notified)}<p role="status">{t.agents.targetUnavailable}</p>{/if}
    {#if detailOpen}
      {#if !data}<p class="source">{t.agents.noReading}</p>{:else if shown.length === 0}<p class="source">{t.agents.empty}</p>{/if}
    {/if}
    {#if data}
      <ul class="sessions" aria-label={t.agents.list} bind:this={list}>
        {#each shown as session (session.id)}
          {@const why = reason(session)}
          <li data-session-id={session.id} data-state={session.state} aria-current={selected === session.id ? "true" : undefined}>
            <b>{session.project} · <span>{session.name ?? t.agents.unnamed}</span></b>
            <p><span>{providerName(session)}</span> · <span class="state">{t.agents.states[session.state]}</span></p>
            <small>
              {#if why}<span>{why}</span> · {/if}{#if session.branch}<span>{session.branch}</span> · {/if}<span class="num">{elapsed(session, data.observedAt)}</span>
            </small>
            {#if notified === session.id}<small class="tag">{t.agents.notified}</small>{/if}
          </li>
        {/each}
      </ul>
    {/if}
  </dialog>
</div>

<style>
  .agents { display: flex; flex-direction: column; margin-top: 18px; min-width: 0; }
  /* An absent reading keeps the place of the ring, legend and notice, growing with the dials. */
  .agents { --absent-min: calc(var(--dial, 94px) + 110px + var(--rest, 0px)); }
  .readings[data-stale="true"] { opacity: .45; }
  .link { color: var(--accent); text-decoration: underline; font-size: inherit; }
  .waiting .see { align-self: flex-start; text-align: left; }
  small { font-size: .88em; color: var(--muted); }
  h3 { font-size: 1em; font-weight: 500; }
  .line { display: block; margin-bottom: 6px; overflow-wrap: anywhere; }
  .line.alert { color: var(--warn); }
  .summary { display: grid; grid-template-columns: 1fr 1fr; gap: 18px 12px; align-items: center; }
  .agent-instrument { text-align: center; min-width: 0; }
  /* --dial grows with the free height of the card (Panel); the legend spreads with it, and
     with --rest, the height left once the dials reach their largest. */
  .session-dial { position: relative; width: min(var(--dial, 94px), 100%); aspect-ratio: 1; margin: auto; display: block; container-type: inline-size; }
  .session-dial svg { width: 100%; height: 100%; display: block; }
  /* Near the B size at 94 px, growing at a sixth of the dial so big rings keep a light number. */
  .reading { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; font-size: calc(31px + (100cqi - 94px) / 6); }
  .base { fill: none; stroke: var(--line); stroke-width: 8; }
  .orbit { fill: none; stroke: var(--control-line); stroke-width: 1; stroke-dasharray: 24 12 6 30; transform-origin: center; }
  .legend-list { align-self: center; min-width: 0; }
  .legend-item { position: relative; }
  .legend { width: 100%; display: flex; gap: 7px; align-items: center; padding: calc(4px + (var(--dial, 94px) - 94px) / 10 + var(--rest, 0px) / 10) 0; text-align: left; font-size: .92em; }
  .legend i { width: 6px; height: 6px; border-radius: 50%; background: var(--state-color); flex-shrink: 0; }
  .legend b { margin-left: auto; }
  .session-tip { display: none; position: absolute; z-index: 4; top: 100%; right: 0; width: 240px; overflow-wrap: anywhere; background: var(--surface-2); color: var(--text); padding: 9px; border: 1px solid var(--control-line); box-shadow: 0 4px 20px #0006; font-size: 12px; font-weight: 400; }
  .legend-item:hover .session-tip, .legend-item:focus-within .session-tip { display: block; }
  .waiting { color: var(--warn); border: 1px solid var(--warn); display: flex; flex-direction: column; gap: 5px; padding: 8px 10px; width: 100%; margin: 25px 0 4px; text-align: left; }
  .waiting .see { font-size: .9em; }
  .waiting.calm { color: var(--muted); border-color: var(--line); }
  .waiting.calm .see { color: var(--accent); }
  .session-dial:focus-visible, .legend:focus-visible, .waiting:focus-visible, .see:focus-visible, .link:focus-visible, .detail-button:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }

  dialog { margin: auto; width: min(680px, 90vw); max-height: 80vh; overflow: auto; background: var(--surface); color: var(--text); border: 1px solid var(--control-line); padding: 20px; font-size: 13px; }
  dialog::backdrop { background: #000a; }
  dialog header { display: flex; align-items: center; justify-content: space-between; gap: 10px; min-height: 28px; }
  dialog h2 { font-size: 1.25em; font-weight: 500; text-transform: uppercase; letter-spacing: 1px; }
  .detail-button { color: var(--accent); border-bottom: 1px solid var(--line); padding: 7px 0; }
  .source { margin-top: 12px; color: var(--muted); }
  .sessions { list-style: none; }
  li { margin-top: 12px; border-top: 1px solid var(--line); padding-top: 10px; overflow-wrap: anywhere; display: flex; flex-direction: column; scroll-margin: 60px 0 16px; }
  li p { margin: 8px 0 4px; }
  li[aria-current] { outline: 2px solid var(--accent); outline-offset: 2px; background: var(--accent-dim); }
  [data-state="working"] .state { color: var(--accent); }
  [data-state="waiting"] .state { color: var(--warn); }
  [data-state="done"] .state { color: var(--ok); }
  [data-state="failed"] .state { color: var(--bad); }
  [data-state="closed"] .state { color: var(--band-lavender-text); }
  .tag { color: var(--accent); margin-top: 4px; }
</style>
