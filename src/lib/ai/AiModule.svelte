<script lang="ts">
  import { onMount } from "svelte";
  import type { AiData, AiReading, AiWindow } from "../core";
  import { t, interpolate } from "../strings";
  import AbsentArea, { type Absence } from "../AbsentArea.svelte";
  import { installed, usageWindows } from "./profile";

  /** Renders the top of the "IA e Agentes" card: its shared heading and the AI instruments. */
  let { data, refresh, configure = () => {}, reconnect = () => {} }:
    { data: AiData; refresh: () => void; configure?: () => void; reconnect?: () => void } = $props();
  let now = $state(Date.now());
  let tipsDismissed = $state(false);
  let titleActions: HTMLDivElement;

  /** Manual refreshes stay at least ten seconds apart. */
  let allowedAt = $state(0);
  onMount(() => {
    const timer = setInterval(() => (now = Date.now()), 1000);
    function dismissDescription(event: KeyboardEvent) {
      if (event.key !== "Escape" || tipsDismissed || !titleActions ||
          !(titleActions.contains(document.activeElement) || titleActions.matches(":hover"))) return;
      tipsDismissed = true;
      event.preventDefault();
      event.stopPropagation();
    }
    window.addEventListener("keydown", dismissDescription, true);
    return () => {
      clearInterval(timer);
      window.removeEventListener("keydown", dismissDescription, true);
    };
  });

  const relative = new Intl.RelativeTimeFormat("pt-BR", { numeric: "auto" });
  const clock = new Intl.DateTimeFormat("pt-BR", { hour: "2-digit", minute: "2-digit" });
  const dayAndClock = new Intl.DateTimeFormat("pt-BR", { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });
  const date = new Intl.DateTimeFormat("pt-BR", { day: "2-digit", month: "2-digit", year: "numeric" });

  function age(when: number) {
    const seconds = Math.max(0, Math.floor((now - when) / 1000));
    if (seconds < 60) return t.ai.now;
    if (seconds < 3600) return relative.format(-Math.floor(seconds / 60), "minute");
    if (seconds < 86400) return relative.format(-Math.floor(seconds / 3600), "hour");
    return relative.format(-Math.floor(seconds / 86400), "day");
  }

  /** "Leitura HH:MM"; a reading older than a day also names its date. */
  function readAt(when: number) {
    return interpolate(t.ai.readAt, { time: (now - when >= 86_400_000 ? dayAndClock : clock).format(when) });
  }

  /** At most this much allowance remains when usage is read aloud as high. */
  const HIGH_USAGE_REMAINING = 15;

  function valueText(percent: number) {
    const remaining = interpolate(t.ai.remainingValue, { percent: String(percent) });
    return percent <= HIGH_USAGE_REMAINING ? `${remaining} · ${t.ai.high}` : remaining;
  }

  function restarted(window: AiWindow) {
    return window.reset || (window.resetsAt !== null && window.resetsAt <= now);
  }

  /** An absent or restarted window has no known remaining allowance. */
  function remainingPercent(window: AiWindow | null) {
    if (!window || restarted(window) || window.usedPercent === null) return null;
    return Math.max(0, Math.min(100, Math.round(100 - window.usedPercent)));
  }
  /** Why a window has no value: no reading, or restarted without a new one. */
  function missing(window: AiWindow | null) {
    return !window || !restarted(window) || window.resetsAt === null
      ? t.ai.noReading : interpolate(t.ai.restarted, { time: clock.format(window.resetsAt) });
  }

  const actions = {
    configure: { label: t.ai.configure, run: () => configure() },
    reconnect: { label: t.ai.reconnect, run: () => reconnect() },
  };
  /** A conditional line in the card body; alerts need the user. */
  interface Notice { text: string; alert?: boolean; action?: keyof typeof actions }
  interface Provider {
    name: string;
    /** Ring colour: always the provider's, whatever the usage. */
    tone: string;
    reading: AiReading | null;
    /** Without a reading, the provider's column is one area saying why. */
    absence: Absence | null;
    notices: Notice[];
    /** The reading shown is older than a failed refresh: dimmed, in the provider's own colour. */
    old?: boolean;
  }

  function failure(since: number | null | undefined, paused: boolean | undefined): Notice[] {
    return since != null && !paused ? [{ text: interpolate(t.ai.failureAge, { age: age(since) }), alert: true }] : [];
  }
  const pausedNotice = (paused: boolean | undefined): Notice[] => (paused ? [{ text: t.ai.paused }] : []);
  /** A failure before any reading offers a new attempt, under the same limits as ↻. */
  function failedFirst(since: number): Absence {
    return {
      text: interpolate(t.ai.failureNoReading, { age: age(since) }),
      tone: "warn",
      action: { label: t.absent.tryAgain, run: refreshNow, disabled: data.paused || data.refreshing || now < allowedAt },
    };
  }

  function codexOf(codex: AiData["codex"]): Provider {
    const base = { name: t.ai.codex, tone: "var(--accent)" };
    if (codex.state === "missing") return { ...base, reading: null, absence: { text: t.ai.codexNeedsSession, tone: "warn" }, notices: [] };
    if (!codex.reading) return {
      ...base, reading: null, notices: pausedNotice(data.paused),
      absence: codex.failureSince != null && !data.paused ? failedFirst(codex.failureSince) : { text: t.ai.emptyCodex, tone: "neutral" },
    };
    const failed = failure(codex.failureSince, data.paused);
    return { ...base, reading: codex.reading, absence: null, notices: [...failed, ...pausedNotice(data.paused)], old: failed.length > 0 };
  }

  /** Each provider is independent: Claude's state never hides Codex. */
  function claudeOf(claude: NonNullable<AiData["claude"]>): Provider {
    const base = { name: t.ai.claude, tone: "var(--band-lavender-text)" };
    const off = claude.statusLine === "off" && claude.usage !== "on";
    if (!claude.reading) {
      const failing = claude.usage === "on" && claude.failureSince != null && !claude.paused;
      const absence: Absence = claude.usageUnavailable ? { text: t.ai.usageUnavailable, tone: "warn", action: { label: t.ai.configure, run: () => configure() } }
        : claude.usage === "expired" ? { text: t.ai.expired, tone: "warn" }
        : claude.statusLine === "replaced" ? { text: t.ai.replaced, tone: "warn", action: { label: t.absent.reconnect, run: () => reconnect() } }
        : failing ? failedFirst(claude.failureSince!)
        : off ? { text: t.ai.offClaude, tone: "neutral", action: { label: t.absent.configure, run: () => configure() } }
        : { text: claude.statusLine === "connected" ? t.ai.emptyClaude : t.ai.noReadingClaude, tone: "neutral" };
      return { ...base, reading: null, absence, notices: pausedNotice(claude.paused) };
    }
    const notices: Notice[] = pausedNotice(claude.paused);
    const failed = claude.usage === "on" ? failure(claude.failureSince, claude.paused) : [];
    if (claude.usageUnavailable) notices.push({ text: t.ai.usageUnavailable, alert: true, action: "configure" });
    if (claude.usage === "expired") notices.push({ text: t.ai.expired, alert: true });
    else notices.push(...failed);
    if (claude.statusLine === "replaced") notices.push({ text: t.ai.replaced, alert: true, action: "reconnect" });
    if (off) notices.push({ text: t.ai.offClaude, action: "configure" });
    return { ...base, reading: claude.reading, absence: null, notices, old: failed.length > 0 || claude.usageUnavailable };
  }

  function refreshNow() {
    allowedAt = Date.now() + 10000;
    refresh();
  }

  const providers = $derived.by(() => {
    const visible: Provider[] = [];
    if (installed(data.codex)) visible.push(codexOf(data.codex));
    if (data.claude && installed(data.claude)) visible.push(claudeOf(data.claude));
    return visible;
  });
</script>

{#snippet rings(arc: number | null)}
  <svg viewBox="0 0 120 120" aria-hidden="true">
    <circle class="orbit" cx="60" cy="60" r="55" />
    <circle class="base" cx="60" cy="60" r="47" />
    {#if arc !== null}
      <circle class="arc" cx="60" cy="60" r="47" pathLength="100" stroke-dasharray="{arc} 100" transform="rotate(-90 60 60)" />
    {/if}
  </svg>
{/snippet}

{#snippet dial(name: string, window: AiWindow | null, percent: number | null)}
  {#if percent === null}
    <div class="dial" role="img" aria-label="{name}: {missing(window)}">
      {@render rings(null)}<span class="reading num" aria-hidden="true">—</span>
    </div>
  {:else}
    <div class="dial" role="meter" aria-label={name} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent}
      aria-valuetext={valueText(percent)}>
      {@render rings(percent)}<span class="reading num">{percent}<small>%</small></span>
    </div>
  {/if}
{/snippet}

{#snippet notice(n: Notice)}
  <small role="status" class="line" class:alert={n.alert}>{#if n.alert}<span aria-hidden="true">{"⚠ "}</span>{/if}{n.text}{#if n.action}{" · "}<button class="link" onclick={actions[n.action].run}>{actions[n.action].label}</button>{/if}</small>
{/snippet}

<header>
  <h2>{t.intelligence.title}</h2>
  <div class="actions" bind:this={titleActions}>
    <span class="action-wrap" role="group" onpointerenter={() => tipsDismissed = false} onfocusin={() => tipsDismissed = false}>
      <button class="icon" aria-label={t.ai.refresh} aria-describedby="ai-refresh-description"
        disabled={data.paused || data.refreshing || now < allowedAt} onclick={refreshNow}><span aria-hidden="true">↻</span></button>
      <span class="tip" hidden={tipsDismissed} id="ai-refresh-description" role="tooltip">{t.ai.refreshDescription}</span>
    </span>
    <span class="action-wrap" role="group" onpointerenter={() => tipsDismissed = false} onfocusin={() => tipsDismissed = false}>
      <button class="icon" aria-label={t.ai.configureAi} aria-describedby="ai-configure-description" onclick={configure}><span aria-hidden="true">⚙</span></button>
      <span class="tip" hidden={tipsDismissed} id="ai-configure-description" role="tooltip">{t.ai.configureDescription}</span>
    </span>
  </div>
</header>
{#if data.refreshing}<small role="status" class="line">{t.ai.refreshing}</small>{/if}

{#if providers.length === 0}
  <p class="line" role="status">{data.codex.installed === null || data.claude?.installed === null
    ? t.ai.checkingTools : t.ai.noTools}</p>
{/if}
<div class="instruments" class:single={providers.length === 1}>
  {#each providers as provider (provider.name)}
    {#if provider.absence}
      <section class="provider absent" aria-label={provider.name}>
        <h3>{provider.name}</h3>
        {#each provider.notices as n (n.text)}{@render notice(n)}{/each}
        <AbsentArea absence={provider.absence} />
      </section>
    {:else}
      {@const { five, week } = usageWindows(provider.reading)}
      {@const fivePercent = remainingPercent(five)}
      {@const weekPercent = remainingPercent(week)}
      {@const weeklyOnly = five === null && week !== null}
      <section class="provider" aria-label={provider.name} style:--instrument={provider.tone} data-stale={provider.old}>
        <div class="instrument">
          {#if weeklyOnly}
            {@render dial(interpolate(t.ai.weeklyOf, { provider: provider.name }), week, weekPercent)}
          {:else if five}
            {@render dial(interpolate(t.ai.fiveHourOf, { provider: provider.name }), five, fivePercent)}
          {/if}
          <h3>{provider.name}</h3>
          {#if weeklyOnly}
            <small>{t.ai.weeklyLabel}</small>
            {#if weekPercent === null}
              <small class="line">{missing(week) === t.ai.noReading ? t.ai.weeklyUnavailable : missing(week)}</small>
            {:else if week.resetsAt != null}
              <small>{interpolate(t.ai.weeklyResets, { date: date.format(week.resetsAt) })}</small>
            {/if}
          {:else if five}
            <small>{fivePercent === null ? t.ai.fiveHour : five.resetsAt == null ? t.ai.fiveHourRemaining
              : interpolate(t.ai.fiveHourResets, { time: clock.format(five.resetsAt) })}</small>
            {#if fivePercent === null}<small class="line">{missing(five)}</small>{/if}
          {:else}
            <small class="line">{t.ai.noReading}</small>
          {/if}
        </div>
        {#if week && !weeklyOnly}
        <div class="meter">
          <div class="meter-label"><span>{t.ai.weeklyLabel}</span><b class="num">{weekPercent === null ? "—" : `${weekPercent}%`}</b></div>
          {#if weekPercent !== null}
            <div class="track" role="meter" aria-label={interpolate(t.ai.weeklyOf, { provider: provider.name })}
              aria-valuemin={0} aria-valuemax={100} aria-valuenow={weekPercent} aria-valuetext={valueText(weekPercent)}>
              <i style:width="{weekPercent}%"></i>
            </div>
            {#if week?.resetsAt != null}<small>{interpolate(t.ai.weeklyResets, { date: date.format(week.resetsAt) })}</small>{/if}
          {:else}
            <div class="track" aria-hidden="true"></div>
            <small>{missing(week) === t.ai.noReading ? t.ai.weeklyUnavailable : missing(week)}</small>
          {/if}
        </div>
        {/if}
        {#if provider.reading}
          <small class="source"><time datetime={new Date(provider.reading.readAt).toISOString()}>{readAt(provider.reading.readAt)}</time></small>
        {/if}
        <div class="notices">{#each provider.notices as n (n.text)}{@render notice(n)}{/each}</div>
      </section>
    {/if}
  {/each}
</div>

<style>
  header { display: flex; align-items: center; gap: 10px; justify-content: space-between; min-height: 28px; margin-bottom: 6px; }
  h2 { font-size: 1.25em; font-weight: 500; text-transform: uppercase; letter-spacing: 1px; }
  h3 { font-size: 1em; font-weight: 500; }
  small { font-size: .88em; color: var(--muted); }
  .actions { display: flex; gap: 5px; }
  .action-wrap { position: relative; display: inline-flex; }
  .icon { width: 28px; height: 28px; border: 1px solid var(--line); border-radius: 50%; font-size: 19px; line-height: 1; display: grid; place-items: center; }
  .icon:hover:not(:disabled) { background: var(--surface-2); }
  .icon:disabled { opacity: .45; cursor: default; }
  .tip { position: absolute; z-index: 4; right: 0; top: 100%; min-width: 150px; background: var(--surface-2); color: var(--text); padding: 9px; border: 1px solid var(--control-line); box-shadow: 0 4px 20px #0006; font-size: 12px; font-weight: 400; visibility: hidden; }
  .action-wrap:hover .tip, .action-wrap:focus-within .tip { visibility: visible; }
  .instruments { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); column-gap: 12px; align-items: start; }
  .instruments.single { grid-template-columns: minmax(0, 1fr); }
  /* Shared rows let either explanation grow without moving only one weekly meter. */
  .provider { min-width: 0; display: grid; grid-row: span 4; grid-template-rows: subgrid; }
  /* An absent provider keeps its column: as tall as the other one, or as a B column with the current dial. */
  .provider.absent { display: flex; flex-direction: column; align-self: stretch; --absent-min: calc(var(--dial, 94px) + 96px); }
  .provider.absent h3 { text-align: center; margin-bottom: 6px; }
  .provider[data-stale="true"] :is(.dial, .meter) { opacity: .45; }
  .instrument { grid-row: 1; align-self: start; text-align: center; display: flex; flex-direction: column; }
  .meter { grid-row: 2; }
  .source { grid-row: 3; }
  .notices { grid-row: 4; }
  /* --dial grows with the free height of the card (Panel); never wider than the column. */
  .dial { position: relative; width: min(var(--dial, 94px), 100%); aspect-ratio: 1; margin: auto; container-type: inline-size; }
  .dial svg { width: 100%; height: 100%; display: block; }
  /* Near the B size at 94 px, growing at a sixth of the dial so big rings keep a light number. */
  .reading { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; font-size: calc(31px + (100cqi - 94px) / 6); gap: 2px; }
  .reading small { font-size: .4em; }
  .base { fill: none; stroke: var(--line); stroke-width: 8; }
  .arc { fill: none; stroke: var(--instrument); stroke-width: 8; }
  .orbit { fill: none; stroke: var(--control-line); stroke-width: 1; stroke-dasharray: 24 12 6 30; transform-origin: center; }
  .meter { margin-top: 6px; display: flex; flex-direction: column; }
  .meter-label { display: flex; justify-content: space-between; gap: 8px; margin-bottom: 4px; }
  .track { height: 5px; background: var(--line); border-radius: 5px; }
  .track i { display: block; height: 100%; background: var(--accent); border-radius: inherit; }
  .meter small { margin-top: 4px; }
  .source { display: block; margin-top: 4px; }
  .line { display: block; margin-top: 4px; overflow-wrap: anywhere; }
  .line.alert { color: var(--warn); }
  .link { color: var(--accent); text-decoration: underline; font-size: inherit; }
  .icon:focus-visible, .link:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
</style>
