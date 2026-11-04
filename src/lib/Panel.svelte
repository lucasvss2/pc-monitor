<script lang="ts">
  import { onMount, tick, untrack, type Snippet } from "svelte";
  import { MODULES, type AgentsData, type AiData, type GithubData, type TrelloData, type NetworkData, type SystemData, type Module, type Core, type Snapshot } from "./core";
  import OperationsStrip from "./OperationsStrip.svelte";
  import AgentsModule from "./agents/AgentsModule.svelte";
  import AiModule from "./ai/AiModule.svelte";
  import { BASE_DIAL, dialSize } from "./instrumentFill";
  import GithubModule from "./github/GithubModule.svelte";
  import NetworkModule from "./network/NetworkModule.svelte";
  import SystemModule from "./system/SystemModule.svelte";
  import TrelloModule from "./trello/TrelloModule.svelte";
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

  /** First opening: Settings shows the welcome until a section is chosen or the user goes back to the panel. */
  let welcome = $state(false);
  const appearance = new AppearanceState();
  let nativeVisible = $state(false);
  let appearanceRestored = $state(false);
  let pageVisible = $state(!document.hidden);
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

  let scroller: HTMLElement;
  let intelligence: HTMLElement;
  let settings = $state<ReturnType<typeof Settings>>();
  const sections: Partial<Record<Module, HTMLElement>> = {};
  /** Element focused before opening Settings, focused again on return. */
  let returnFocus: HTMLElement | null = null;

  /** Modules intersecting the visible panel area, kept outside reactivity. */
  const intersecting = new Set<Module>();
  let reported: string | null = null;

  function receive(s: Snapshot) {
    const previous = snapshots[s.module];
    if (!previous || (previous.updatedAt ?? 0) <= (s.updatedAt ?? 0)) snapshots[s.module] = s;
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

  /** Open Settings, at the section of module when given. */
  async function openSettings(module?: Module) {
    if (area !== "settings") {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      area = "settings";
      syncVisibility();
      await tick();
    }
    await settings?.focus(module);
  }

  /** The panel stays mounted under Settings, so scroll and module state survive. */
  async function backToPanel() {
    welcome = false;
    await showPanel();
    if (returnFocus?.isConnected) returnFocus.focus({ preventScroll: true });
    returnFocus = null;
  }

  async function target(session: string) {
    agentTarget = session;
    agentRequest += 1;
    await showPanel();
    await tick();
    reveal("agents", false);
  }

  const NO_AI_READING: AiData = { refreshing: false, codex: { state: "empty", plan: null, reading: null } };
  const bodies: Record<Module, Snippet> = { ai, agents, github, trello, network, system };

  const supportingModules: Module[] = ["github", "network", "trello", "system"];
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

  onMount(() => {
    let observer: IntersectionObserver | undefined;
    if (typeof IntersectionObserver === "function") {
      observer = new IntersectionObserver((entries) => {
        for (const entry of entries) {
          const module = (entry.target as HTMLElement).dataset.module as Module;
          if (entry.isIntersecting) intersecting.add(module);
          else intersecting.delete(module);
        }
        syncVisibility();
      }, { root: scroller });
      for (const section of Object.values(sections)) observer.observe(section);
    } else {
      MODULES.forEach((m) => intersecting.add(m));
    }

    // Subscribe before requesting initial state to avoid losing snapshots in between.
    let disposed = false;
    let visibilityReceived = false;
    const stopVisibility = core.listenPanelVisibility?.((visible) => {
      if (disposed) return;
      visibilityReceived = true;
      nativeVisible = visible;
      syncVisibility();
    });
    const stop = Promise.resolve().then(() => core.listenSnapshots((snapshot) => { if (!disposed) receive(snapshot); }));
    const stopTarget = core.listenAgentTarget?.((session) => { if (!disposed) void target(session); });
    const stopMedia = appearance.watch();
    const stopTheme = core.listenSystemTheme?.((theme) => { if (!disposed) appearance.desktop = theme; });
    core.initialState().then(async (initial) => {
      if (disposed) return;
      if (!visibilityReceived) nativeVisible = initial.panelVisible ?? false;
      initial.snapshots.forEach(receive);
      unavailableModules = initial.unavailableModules ?? [];
      if (initial.appearance) appearance.preference = initial.appearance;
      appearance.desktop ??= initial.systemTheme ?? null;
      appearanceRestored = true;
      agentTarget ??= initial.agentTarget ?? null;
      anchor ??= initial.lastAnchor;
      await tick();
      if (disposed) return;
      if (area === "panel") reveal(agentTarget ? "agents" : anchor, false);
      syncVisibility();
      if (initial.welcome && !agentTarget) {
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
      observer?.disconnect();
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

{#snippet trello()}
  <TrelloModule data={(snapshots.trello?.data as TrelloData | undefined) ?? null} refresh={() => core.refreshTrello?.()}
    configure={() => openSettings("trello")} openCard={(url) => core.openTrelloCard?.(url)}
    moveCard={(card, list) => core.moveTrelloCard?.(card, list) ?? Promise.reject(new Error("unavailable"))} />
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

<div class="panel">
  <header class="toolbar">
    <div class="bridge"><span></span><span></span><span></span><div class="signature"><strong>KVOJPS</strong></div></div>
    <div class="utility">
      <button class="settings" aria-current={area === "settings" ? "page" : undefined} onclick={() => openSettings()}><span aria-hidden="true">⚙</span> {t.settings.title}</button>
    </div>
  </header>

  {#if recovery}<p role="status">Preferências não restauradas; usando valores padrão.</p>{/if}
  <div class="workspace">
    <main class="scroller" bind:this={scroller} class:covered={area === "settings"}
      inert={area === "settings"} aria-hidden={area === "settings" ? "true" : undefined}>
      <div class="composition">
        <div class="card intelligence" bind:this={intelligence}>
          {@render moduleSection("ai", bodies.ai)}
          {@render moduleSection("agents", bodies.agents)}
        </div>
        {#each supportingModules as module (module)}
          {@render moduleSection(module, bodies[module])}
        {/each}
      </div>
    </main>

    {#if area === "settings"}
      <Settings bind:this={settings} {core} {snapshots} {welcome} {appearance} {unavailableModules} back={backToPanel} />
    {/if}
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

  .toolbar { flex: none; }
  .bridge { position: relative; display: flex; gap: 7px; height: 6px; margin-bottom: 5px; }
  .signature { position: absolute; top: -6px; left: 22px; height: 16px; padding: 0 10px; background: var(--bg); transform: skew(-12deg); }
  .signature strong { display: block; font: 600 11px monospace; letter-spacing: .2em; transform: skew(12deg); color: var(--accent); }
  .bridge span { background: var(--band-lavender); flex: 2; border-radius: 15px 0 0 0; }
  .bridge span:nth-child(2) { background: var(--band-cyan); flex: 5; border-radius: 0; }
  .bridge span:nth-of-type(3) { background: var(--band-amber); flex: 1; border-radius: 0 12px 12px 0; }
  .utility { display: flex; justify-content: flex-end; align-items: center; margin-bottom: 6px; min-height: 24px; }
  .settings { display: flex; align-items: center; gap: 7px; font-size: 13px; }
  .settings span { font-size: 16px; line-height: 1; }
  .settings[aria-current] { color: var(--accent); }
  .settings:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  .workspace { position: relative; flex: 1; min-height: 0; }
  .scroller { height: 100%; overflow: auto; }
  .scroller.covered { visibility: hidden; }
  /* The grid takes the whole Panel height: no empty strip at the bottom of the screen. */
  .composition { min-height: 100%; display: grid; grid-template-columns: minmax(0, 1.2fr) repeat(2, minmax(0, 1fr)); grid-template-areas: "intelligence github network" "intelligence trello system"; gap: 12px; align-items: stretch; }
  .intelligence { grid-area: intelligence; display: flex; flex-direction: column; }
  .composition > :global(.github) { grid-area: github; }
  .composition > :global(.network) { grid-area: network; }
  .composition > :global(.trello) { grid-area: trello; }
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
  .card.trello { --module-band: var(--band-amber); }
  .card.network { --module-band: var(--band-blue); }
  .module { min-width: 0; display: flex; flex-direction: column; scroll-margin-top: 4px; overflow-wrap: anywhere; }
  .module:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  @media (max-width: 950px) {
    .composition { grid-template-columns: repeat(2, minmax(0, 1fr)); grid-template-areas: "intelligence intelligence" "github network" "trello system"; }
  }
  @media (max-width: 650px) {
    .panel { padding: 4px; }
    .composition { grid-template-columns: minmax(0, 1fr); grid-template-areas: "intelligence" "github" "network" "trello" "system"; }
  }
</style>
