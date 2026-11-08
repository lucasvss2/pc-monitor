<script lang="ts">
  import { onMount, tick, untrack, type Snippet } from "svelte";
  import { MODULES, type AgentsData, type AiData, type GithubData, type TasksData, type NetworkData, type SystemData, type Module, type Core, type Snapshot, type PanelMode, type Presentation } from "./core";
  import CompactPanel from "./CompactPanel.svelte";
  import { formatPanelClock } from "./panelClock";
  import OperationsStrip from "./OperationsStrip.svelte";
  import AgentsModule from "./agents/AgentsModule.svelte";
  import AiModule from "./ai/AiModule.svelte";
  import { BASE_DIAL, dialSize } from "./instrumentFill";
  import GithubModule from "./github/GithubModule.svelte";
  import NetworkModule from "./network/NetworkModule.svelte";
  import SystemModule from "./system/SystemModule.svelte";
  import TasksModule from "./tasks/TasksModule.svelte";
  import { AppearanceState } from "./settings/appearance.svelte";
  import Settings from "./settings/Settings.svelte";
  import { t } from "./strings";

  let { core, prepared = () => {}, failed = () => {}, recovery = false, presented = true }: { core: Core; prepared?: () => void; failed?: () => void; recovery?: boolean; presented?: boolean } = $props();

  let area = $state<"panel" | "settings">("panel");
  let unavailableModules = $state<Module[]>([]);
  /** Last anchor chosen through module interaction; independent from what is visible. */
  let anchor = $state<Module | null>(null);
  let agentRequest = $state(0);
  let agentTarget = $state<string | null>(null);
  let snapshots = $state<Partial<Record<Module, Snapshot>>>({});
  let presentation = $state<Presentation | null>(null);
  const compact = $derived(presentation?.mode === "compact");
  const returningToCompact = $derived(!compact && presentation?.preferredMode === "compact");
  let windowBusy = $state(false);
  let windowError = $state("");
  let modeButton: HTMLButtonElement | undefined = $state();
  let nextModeChange = Promise.resolve(true);
  let civil = $state<number | null>(null);
  let clockTick = $state(Date.now());
  const clockText = $derived(civil === null ? "—" : formatPanelClock(civil));
  const [clockTime, clockDate] = $derived(clockText.split(" · "));

  /** First opening: Settings shows the welcome until a section is chosen or the user goes back to the panel. */
  let welcome = $state(false);
  let preferencesError = $state<string | null>(null);
  const appearance = new AppearanceState();
  let nativeVisible = $state(false);
  let appearanceRestored = $state(false);
  let pageVisible = $state(!document.hidden);
  $effect(() => {
    if (!presented || !pageVisible || !nativeVisible) return;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const read = async () => {
      const local = new Date();
      try {
        const value = await (core.panelClock?.() ?? Promise.resolve(local.getTime() - local.getTimezoneOffset() * 60_000));
        if (!stopped) { civil = value; clockTick = local.getTime(); }
      } catch { if (!stopped) civil = null; }
      if (!stopped) timer = setTimeout(() => void read(), 1000 - Date.now() % 1000);
    };
    void read();
    return () => { stopped = true; clearTimeout(timer); };
  });
  $effect(() => {
    const state = presented && appearanceRestored && nativeVisible && pageVisible && area === "panel" && !appearance.reduced ? "running" : "paused";
    document.documentElement.dataset.decoration = state;
    // Bounded in-memory trace: no source data or per-frame callbacks.
    performance.clearMarks?.("panel-decoration");
    performance.mark?.("panel-decoration", { detail: { state } });
  });
  $effect(() => {
    document.documentElement.dataset.theme = appearance.theme;
    document.documentElement.dataset.motion = appearance.reduced ? "reduced" : "full";
  });

  let scroller: HTMLElement = $state()!;
  let intelligence: HTMLElement;
  let settings = $state<ReturnType<typeof Settings>>();
  const sections: Partial<Record<Module, HTMLElement>> = {};
  /** Element focused before opening Settings, focused again on return. */
  let returnFocus: HTMLElement | null = null;

  /** Modules intersecting the visible panel area, kept outside reactivity. */
  const intersecting = new Set<Module>();
  let reported: string | null = null;

  let runtimeId: string | undefined;
  let bootstrapped = false;
  const pendingSnapshots: Snapshot[] = [];
  function receive(s: Snapshot) {
    // Legacy snapshots exist only in UI fixtures; native events wait for runtime identity.
    if (!bootstrapped && s.runtimeId) { pendingSnapshots.push(s); return; }
    if (runtimeId && s.runtimeId !== runtimeId) return;
    const previous = snapshots[s.module];
    if (previous?.revision !== undefined && s.revision !== undefined && s.revision <= previous.revision) return;
    snapshots[s.module] = s;
  }

  /** Report the visible set to the core only when it changes. */
  function syncVisibility() {
    const modules = presented && area === "panel" && nativeVisible && pageVisible ? MODULES.filter((m) => intersecting.has(m)) : [];
    const key = modules.join();
    if (key === reported) return;
    reported = key;
    core.reportVisibleModules(modules);
  }

  function reveal(module: Module, focus: boolean) {
    const section = sections[module];
    if (!section) return;
    section.scrollIntoView({ block: "start", behavior: focus && !appearance.reduced ? "smooth" : "auto" });
    if (focus) section.focus({ preventScroll: true });
  }

  async function showPanel() {
    if (area === "panel") return;
    area = "panel";
    await tick();
    syncVisibility();
  }

  function remember(module: Module, event: FocusEvent) {
    if ((event.target as HTMLElement).closest("dialog")) return;
    agentTarget = null;
    anchor = module;
    if (area === "panel") core.chooseAnchor(module);
  }

  function changeMode(mode: PanelMode, remember: boolean) {
    nextModeChange = nextModeChange.then(async () => {
      if (!core.setPanelMode) return true;
      windowBusy = true;
      windowError = "";
      try {
        presentation = await core.setPanelMode(mode, remember);
        return true;
      } catch {
        windowError = t.header.modeError;
        return false;
      } finally { windowBusy = false; }
    });
    return nextModeChange;
  }

  async function toggleMode() {
    const mode = compact ? "full" : "compact";
    if (!await changeMode(mode, !returningToCompact)) return;
    if (mode === "compact") await showPanel();
    await tick();
    modeButton?.focus({ preventScroll: true });
    if (mode === "full" && anchor) reveal(anchor, false);
  }

  async function togglePin() {
    if (!presentation?.pinAvailable || !core.setPanelPinned || windowBusy) return;
    windowBusy = true;
    windowError = "";
    try { presentation = await core.setPanelPinned(!presentation.pinned); }
    catch { windowError = t.header.pinError; }
    finally { windowBusy = false; }
  }

  async function expandModule(module: Module) {
    if (!await changeMode("full", false)) return;
    await tick();
    reveal(module, true);
  }

  /** Open Settings, at the section of module when given. */
  async function openSettings(module?: Module) {
    if (area !== "settings") {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      if (compact && !await changeMode("full", false)) return;
      area = "settings";
      syncVisibility();
      await tick();
    }
    await settings?.focus(module);
  }

  /** The panel stays mounted under Settings, so scroll and module state survive. */
  async function backToPanel() {
    welcome = false;
    if (returningToCompact) await changeMode("compact", false);
    await showPanel();
    if (returnFocus?.isConnected) returnFocus.focus({ preventScroll: true });
    else if (compact) modeButton?.focus({ preventScroll: true });
    returnFocus = null;
  }

  async function target(session: string) {
    if (compact && !await changeMode("full", false)) return;
    agentTarget = session;
    agentRequest += 1;
    await showPanel();
    await tick();
    reveal("agents", false);
  }

  const NO_AI_READING: AiData = { refreshing: false, codex: { installed: null, state: "empty", plan: null, reading: null } };
  const bodies: Record<Module, Snippet> = { ai, agents, github, tasks, network, system };

  const supportingModules: Module[] = ["github", "network", "tasks", "system"];
  const agentsData = $derived((snapshots.agents?.data as AgentsData | undefined) ?? null);

  /** The free height of the "IA e Agentes" card goes to its dials: no empty strip between or after the blocks. */
  function fitInstruments() {
    intelligence.style.setProperty("--dial", `${BASE_DIAL}px`);
    intelligence.style.setProperty("--rest", "0px");
    const style = getComputedStyle(intelligence);
    const inner = intelligence.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom);
    const free = () => inner - Array.from(intelligence.children).reduce((sum, child) => {
      const box = getComputedStyle(child);
      return sum + child.getBoundingClientRect().height + parseFloat(box.marginTop) + parseFloat(box.marginBottom);
    }, 0);
    intelligence.style.setProperty("--dial", `${dialSize(free())}px`);
    // Dials at their largest: what is left goes to the Agents block only (its legend, or
    // its absent area), so no height counts twice. The legend may start shorter than its
    // ring, so measure again until nothing is left.
    let rest = 0;
    for (let pass = 0; pass < 3; pass++) {
      const left = free();
      if (left < 1) break;
      rest += left;
      intelligence.style.setProperty("--rest", `${rest}px`);
    }
  }

  onMount(() => {
    if (typeof ResizeObserver !== "function") return;
    let frame = 0;
    // The card size comes from the grid; its blocks change with conditional lines.
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(fitInstruments);
    });
    observer.observe(intelligence);
    Array.from(intelligence.children).forEach((child) => observer.observe(child));
    fitInstruments();
    return () => { observer.disconnect(); cancelAnimationFrame(frame); };
  });

  $effect(() => {
    if (!scroller) return;
    const reduced = compact;
    intersecting.clear();
    let observer: IntersectionObserver | undefined;
    if (typeof IntersectionObserver === "function") {
      observer = new IntersectionObserver((entries) => {
        for (const entry of entries) {
          const data = (entry.target as HTMLElement).dataset;
          const module = (reduced ? data.compactModule : data.module) as Module;
          if (entry.isIntersecting) intersecting.add(module);
          else intersecting.delete(module);
        }
        syncVisibility();
      }, { root: scroller });
      scroller.querySelectorAll(reduced ? "[data-compact-module]" : "[data-module]").forEach(section => observer!.observe(section));
    } else {
      MODULES.forEach((m) => intersecting.add(m));
    }
    untrack(syncVisibility);
    return () => observer?.disconnect();
  });

  onMount(() => {
    // Subscribe before requesting initial state to avoid losing snapshots in between.
    let disposed = false;
    let visibilityReceived = false;
    const stopVisibility = core.listenPanelVisibility?.((visible) => {
      if (disposed) return;
      visibilityReceived = true;
      nativeVisible = visible;
      syncVisibility();
    });
    const stop = (async () => core.listenSnapshots((snapshot) => { if (!disposed) receive(snapshot); }))();
    const stopTarget = core.listenAgentTarget?.((session) => { if (!disposed) void target(session); });
    const stopMedia = appearance.watch();
    const stopTheme = core.listenSystemTheme?.((theme) => { if (!disposed) appearance.desktop = theme; });
    core.initialState().then(async (initial) => {
      if (disposed) return;
      if (!visibilityReceived) nativeVisible = initial.panelVisible ?? false;
      runtimeId = initial.runtimeId;
      bootstrapped = true;
      initial.snapshots.forEach(receive);
      pendingSnapshots.splice(0).forEach(receive);
      unavailableModules = initial.unavailableModules ?? [];
      preferencesError = initial.preferencesError ?? null;
      if (initial.appearance) appearance.preference = initial.appearance;
      presentation = initial.presentation ?? null;
      appearance.desktop ??= initial.systemTheme ?? null;
      appearanceRestored = true;
      agentTarget ??= initial.agentTarget ?? null;
      anchor ??= initial.lastAnchor;
      await tick();
      if (disposed) return;
      if (area === "panel" && !compact) reveal(agentTarget !== null ? "agents" : anchor, false);
      syncVisibility();
      if (initial.welcome && agentTarget === null) {
        welcome = true;
        core.completeWelcome?.();
        await openSettings();
      }
      if (!disposed) prepared();
    }).catch(() => { if (!disposed) failed(); });
    const subscriptions = [stopVisibility, stop, stopTarget, stopTheme];
    for (const subscription of subscriptions) void subscription?.catch(() => { if (!disposed) failed(); });
    return () => {
      disposed = true;
      document.documentElement.dataset.decoration = "paused";
      void stopVisibility?.then((stop) => stop(), () => {});
      stopMedia();
      void stopTheme?.then((p) => p(), () => {});
      void stop.then((p) => p(), () => {});
      void stopTarget?.then((p) => p(), () => {});
    };
  });

  $effect(() => {
    if (!presented) return;
    untrack(() => {
      syncVisibility();
      if (area === "settings") void settings?.focus();
    });
  });

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape" && !e.defaultPrevented) core.closePanel();
  }
</script>

<svelte:window onkeydown={onKeyDown} />
<svelte:document onvisibilitychange={() => { pageVisible = !document.hidden; syncVisibility(); }} />

{#snippet ai()}
  <!-- Before the first snapshot the instruments explain there is no reading and keep their actions. -->
  <AiModule data={(snapshots.ai?.data as AiData | null) ?? NO_AI_READING} refresh={() => core.refreshAi?.()} configure={() => openSettings("ai")}
    reconnect={() => void (core.connectClaude?.() ?? Promise.reject()).catch(() => openSettings("ai"))} />
{/snippet}

{#snippet agents()}
  <AgentsModule data={agentsData} notified={agentTarget} notificationRequest={agentRequest} configure={() => openSettings("agents")} />
{/snippet}

{#snippet github()}
  <GithubModule data={(snapshots.github?.data as GithubData | undefined) ?? null} refresh={() => core.refreshGithub?.()} configure={() => openSettings("github")} />
{/snippet}

{#snippet tasks()}
  <TasksModule data={(snapshots.tasks?.data as TasksData | undefined) ?? null} generation={snapshots.tasks?.contextGeneration ?? 0}
    {core} configure={() => openSettings("tasks")} />
{/snippet}

{#snippet network()}
  <NetworkModule data={(snapshots.network?.data as NetworkData | undefined) ?? null} runSpeed={() => core.runSpeedTest?.()} />
{/snippet}

{#snippet system()}
  <SystemModule data={(snapshots.system?.data as SystemData | undefined) ?? null} updatedAt={snapshots.system?.updatedAt ?? null} />
{/snippet}

{#snippet moduleSection(module: Module, body?: Snippet)}
  {@const inCard = module === "ai" || module === "agents"}
  <!-- IA and Agents share one visual card but stay separate modules: own section, snapshot and anchor. -->
  <section id="module-{module}" data-module={module} class="module {module}" class:card={!inCard}
    aria-label={inCard ? t.modules[module] : undefined} aria-labelledby={inCard ? undefined : `module-${module}-title`}
    tabindex="-1" onfocusin={(event) => remember(module, event)} bind:this={sections[module]}>
    {#if recovery && !snapshots[module]}<p role="status">Restauração local indisponível. Exibindo os dados disponíveis.</p>{/if}
    {#if unavailableModules.includes(module)}
      <h2 id="module-{module}-title">{t.modules[module]}</h2>
      <p role="status">Indisponível no Windows nesta versão de validação.</p>
    {:else}
      {@render body?.()}
    {/if}
  </section>
{/snippet}

<div class="panel" class:compact>
  <header class="toolbar" data-tauri-drag-region title={t.header.movePanelHint}>
    <div class="bridge" data-tauri-drag-region><span data-tauri-drag-region></span><span data-tauri-drag-region></span><span data-tauri-drag-region></span><div class="signature" data-tauri-drag-region><strong data-tauri-drag-region>KVOJPS</strong></div></div>
    <div class="utility" data-tauri-drag-region>
      {#if presentation}
        {@const modeLabel = compact ? t.header.expand : returningToCompact ? t.header.returnCompact : t.header.reduce}
        <button class="window-control" bind:this={modeButton} aria-label={modeLabel} title={modeLabel} disabled={windowBusy} onclick={toggleMode}>
          <span aria-hidden="true">{compact ? "⤢" : "⤡"}</span>
          <span class="control-label">{modeLabel}</span>
        </button>
        <button class="window-control pin" title={presentation.pinAvailable ? t.header.pin : t.header.pinUnavailable}
          aria-label={presentation.pinAvailable ? t.header.pin : t.header.pinUnavailable} aria-pressed={presentation.pinned}
          disabled={!presentation.pinAvailable || windowBusy} onclick={togglePin}>
          <svg aria-hidden="true" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M16 3 21 8 17 10 14 15 14 18 6 10 9 10 14 7 16 3ZM10 14l-6 6"/></svg>
          <span class="control-label">{t.header.pin}</span>
        </button>
      {/if}
      <button class="settings" title={t.settings.title} aria-label={t.settings.title} aria-current={area === "settings" ? "page" : undefined} onclick={() => openSettings()}><span aria-hidden="true">⚙</span> <span class="control-label">{t.settings.title}</span></button>
      <button class="close" aria-label={t.header.closePanel} title={t.header.closePanelHint} onclick={() => core.closePanel()}>
        <svg aria-hidden="true" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="m6 6 12 12M18 6 6 18" /></svg>
      </button>
    </div>
  </header>

  {#if recovery}<p role="status">Preferências não restauradas; usando valores padrão.</p>{/if}
  {#if windowError || presentation?.pinFailed}<p class="window-error" role="status">{windowError || t.header.pinError}</p>{/if}
  {#if preferencesError}<p role="alert">{preferencesError === "futureSchema" ? "Suas preferências foram criadas por uma versão mais recente. Atualize o aplicativo para alterá-las." : "Não foi possível carregar suas preferências com segurança. O arquivo salvo foi preservado."}</p>{/if}
  <div class="workspace">
    <main class="scroller" bind:this={scroller} class:covered={area === "settings"}
      inert={area === "settings"} aria-hidden={area === "settings" ? "true" : undefined}>
      <div class="composition" hidden={compact} inert={compact}>
        <div class="card intelligence" bind:this={intelligence}>
          {@render moduleSection("ai", bodies.ai)}
          {@render moduleSection("agents", bodies.agents)}
        </div>
        {#each supportingModules as module (module)}
          {@render moduleSection(module, bodies[module])}
        {/each}
      </div>
      {#if compact}<CompactPanel {snapshots} unavailable={unavailableModules} now={clockTick} expand={expandModule} />{/if}
    </main>

    {#if area === "settings"}
      <Settings bind:this={settings} {core} {snapshots} {welcome} {appearance} {unavailableModules} back={backToPanel} />
    {/if}
  </div>
  <div class="panel-clock" role="group" aria-label={t.header.clock}>
    <time aria-label={clockText}>
      <span class="clock-time">{clockTime}</span><span class="clock-separator">{" · "}</span><span class="clock-date">{clockDate}</span>
    </time>
    <span class="clock-source">{t.header.systemTime}</span>
  </div>
  <OperationsStrip {snapshots} ready={presented && appearanceRestored && nativeVisible && pageVisible} />
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 8px 18px 10px;
    background: var(--bg);
    overflow: hidden;
    font-size: 13px;
  }

  .toolbar { flex: none; cursor: move; user-select: none; }
  .utility button { cursor: pointer; }
  .bridge { position: relative; display: flex; gap: 7px; height: 6px; margin-bottom: 5px; }
  .signature { position: absolute; top: -6px; left: 22px; height: 16px; padding: 0 10px; background: var(--bg); transform: skew(-12deg); }
  .signature strong { display: block; font: 600 11px monospace; letter-spacing: .2em; transform: skew(12deg); color: var(--accent); }
  .bridge span { background: var(--band-lavender); flex: 2; border-radius: 15px 0 0 0; }
  .bridge span:nth-child(2) { background: var(--band-cyan); flex: 5; border-radius: 0; }
  .bridge span:nth-of-type(3) { background: var(--band-amber); flex: 1; border-radius: 0 12px 12px 0; }
  .utility { display: flex; justify-content: flex-end; align-items: center; gap: 10px; margin-bottom: 6px; min-height: 28px; }
  .settings { display: flex; align-items: center; gap: 7px; font-size: 13px; }
  .settings span[aria-hidden] { font-size: 16px; line-height: 1; }
  .settings[aria-current] { color: var(--accent); }
  .settings:focus-visible, .close:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  .close { display: grid; place-items: center; width: 34px; height: 28px; padding: 0; border: 1px solid var(--control-line); border-radius: 4px; background: var(--surface); }
  .close:hover { color: var(--accent); border-color: var(--accent); background: var(--surface-2); }
  .window-control { display: flex; align-items: center; gap: 5px; min-height: 28px; padding: 3px 7px; border: 1px solid var(--control-line); border-radius: 4px; background: var(--surface); }
  .window-control:hover:not(:disabled), .pin[aria-pressed="true"] { color: var(--accent); border-color: var(--accent); }
  .window-control:disabled { opacity: .55; cursor: default; }
  .window-control:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .window-error { color: var(--warn); font-size: .9em; }
  .panel-clock { display: flex; flex: none; justify-content: flex-end; flex-wrap: wrap; align-items: baseline; gap: 2px 8px; padding-top: 4px; }
  .panel-clock time { font-variant-numeric: tabular-nums; font-family: monospace; font-weight: 600; }
  .clock-source { font-size: .85em; color: var(--muted); }
  .compact .panel-clock { flex: 1 0 100px; min-height: 100px; container-type: size; flex-direction: column; justify-content: center; align-items: center; gap: 6px; padding: 8px 0; }
  .compact .panel-clock time { display: flex; flex-direction: column; align-items: center; gap: 6px; width: 100%; line-height: 1; }
  .compact .clock-time { font-size: clamp(26px, min(16cqw, 42cqh), 88px); }
  .compact .clock-date { font-size: clamp(16px, min(8cqw, 22cqh), 40px); }
  .compact .clock-separator { display: none; }
  .compact .clock-source { font-size: 11px; }
  .compact .utility { gap: 6px; }
  .compact .settings { width: 32px; height: 28px; justify-content: center; }
  .compact .control-label { display: none; }
  .workspace { position: relative; flex: 1; min-height: 0; }
  .compact .workspace { flex: 0 1 auto; }
  .scroller { height: 100%; overflow: auto; }
  .scroller.covered { visibility: hidden; }
  /* The grid takes the whole Panel height: no empty strip at the bottom of the screen. */
  .composition { min-height: 100%; display: grid; grid-template-columns: minmax(0, 1.2fr) repeat(2, minmax(0, 1fr)); grid-template-areas: "intelligence github network" "intelligence tasks system"; gap: 12px; align-items: stretch; }
  .composition[hidden] { display: none; }
  .intelligence { grid-area: intelligence; display: flex; flex-direction: column; }
  .composition > :global(.github) { grid-area: github; }
  .composition > :global(.network) { grid-area: network; }
  .composition > :global(.tasks) { grid-area: tasks; }
  .composition > :global(.system) { grid-area: system; }

  /* One card per module, except IA and Agents, which share the intelligence card. */
  .card {
    --module-band: var(--band-cyan);
    min-width: 0;
    padding: 8px 14px;
    background: var(--module-bg);
    border-top: 4px solid var(--module-band);
  }
  .card.intelligence { --module-band: var(--band-lavender); }
  .card.github { --module-band: var(--band-steel); }
  .card.system { --module-band: var(--band-lavender); }
  .card.tasks { --module-band: var(--band-amber); }
  .card.network { --module-band: var(--band-blue); }
  .module { min-width: 0; display: flex; flex-direction: column; scroll-margin-top: 4px; overflow-wrap: anywhere; }
  .module:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  @media (max-width: 950px) {
    .composition { grid-template-columns: repeat(2, minmax(0, 1fr)); grid-template-areas: "intelligence intelligence" "github network" "tasks system"; }
  }
  @media (max-width: 650px) {
    .panel { padding: 4px; }
    .composition { grid-template-columns: minmax(0, 1fr); grid-template-areas: "intelligence" "github" "network" "tasks" "system"; }
  }
  @media (max-width: 440px) {
    .utility .control-label { display: none; }
  }
</style>
