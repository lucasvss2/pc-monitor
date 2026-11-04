<script lang="ts">
  import { tick } from "svelte";
  import type { AgentsData, AiData, Core, GithubData, Module, NetworkData, Snapshot, TasksData } from "../core";
  import AgentSettings from "../agents/AgentSettings.svelte";
  import AiSettings from "../ai/AiSettings.svelte";
  import GithubSettings from "../github/GithubSettings.svelte";
  import NetworkSettings from "../network/NetworkSettings.svelte";
  import TasksSettings from "../tasks/TasksSettings.svelte";
  import { t } from "../strings";
  import type { AppearanceState } from "./appearance.svelte";
  import AppearanceSettings from "./AppearanceSettings.svelte";
  import GeneralSettings from "./GeneralSettings.svelte";
  import { moduleStatus } from "./status";
  import Welcome from "./Welcome.svelte";

  let { core, snapshots, welcome, appearance, back, unavailableModules = [] }: {
    unavailableModules?: Module[];
    core: Core;
    snapshots: Partial<Record<Module, Snapshot>>;
    welcome: boolean;
    appearance: AppearanceState;
    back: () => void;
  } = $props();

  /** Sections in the index; System has nothing to configure yet. */
  type Section = "general" | "appearance" | Exclude<Module, "system">;
  const GROUPS: { label: string; sections: Section[] }[] = [
    { label: t.settings.index.app, sections: ["general", "appearance"] },
    { label: t.settings.index.modules, sections: ["ai", "agents", "github", "tasks", "network"] },
  ];
  const TITLES: Record<Section, string> = {
    general: t.settings.general.title,
    appearance: t.settings.appearance.title,
    ai: t.modules.ai,
    agents: t.modules.agents,
    github: t.modules.github,
    tasks: t.modules.tasks,
    network: t.modules.network,
  };
  const isSection = (value: string): value is Section => Object.hasOwn(TITLES, value);
  /** Same rule as the welcome: only modules that need configuration get a mark. */
  const needsSetup = (section: Section) =>
    section !== "general" && section !== "appearance" && !unavailableModules.includes(section) && moduleStatus(section, snapshots[section]) === "needsSetup";

  /** No section yet: the welcome fills the right side on the first opening. */
  let selected = $state<Section | null>(null);
  let area: HTMLElement;
  let pane: HTMLElement;
  let nav: HTMLElement;

  /** Select the section of a module, or General, and focus its title; without a module the welcome stays until a section is chosen. */
  export async function focus(module?: Module) {
    if (!module && welcome && !selected) return focusTitle({ scroll: false });
    await select(module && isSection(module) ? module : "general", { scroll: !!module });
  }

  async function select(section: Section, { scroll = true } = {}) {
    selected = section;
    await focusTitle({ scroll });
  }

  async function focusTitle({ scroll }: { scroll: boolean }) {
    await tick();
    const title = pane.querySelector<HTMLElement>(":scope > section > h3");
    if (!title) return;
    title.tabIndex = -1;
    title.focus({ preventScroll: true });
    if (scroll) title.scrollIntoView({ block: "nearest" });
    else area.scrollTop = 0;
  }

  /** Arrow keys, Home and End move focus between index items; the buttons select as usual. */
  function move(event: KeyboardEvent) {
    const items = [...nav.querySelectorAll<HTMLButtonElement>("button")];
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    if (current < 0) return;
    const next = {
      ArrowDown: current + 1, ArrowRight: current + 1,
      ArrowUp: current - 1, ArrowLeft: current - 1,
      Home: 0, End: items.length - 1,
    }[event.key];
    if (next === undefined) return;
    event.preventDefault();
    items[(next + items.length) % items.length].focus();
  }

  const unavailable = (code: string) => () => Promise.reject(code);
</script>

<div class="settings" bind:this={area} tabindex="-1">
  <header class="settings-head">
    <h2>{t.settings.title}</h2>
    <button class="back" onclick={back}><span aria-hidden="true">← </span>{t.panel.back}</button>
  </header>

  <div class="body">
    <nav aria-label={t.settings.index.label} bind:this={nav}>
      {#each GROUPS as group, g (group.label)}
        <p class="group" id="settings-group-{g}">{group.label}</p>
        <ul aria-labelledby="settings-group-{g}">
          {#each group.sections as section (section)}
            {@const mark = needsSetup(section)}
            <li>
              <button aria-current={selected === section ? "page" : undefined} aria-describedby={mark ? `settings-mark-${section}` : undefined}
                onclick={() => void select(section)} onkeydown={move}>{TITLES[section]}{#if mark}<span class="mark" aria-hidden="true">!</span>{/if}</button>
              {#if mark}<span id="settings-mark-{section}" hidden>{t.settings.index.needsSetup}</span>{/if}
            </li>
          {/each}
        </ul>
      {/each}
    </nav>

    <div class="content">
      <div class="pane {selected ?? 'welcome'}" bind:this={pane}>
        {#if !selected && unavailableModules.length > 0}
          <section><h3>{t.settings.title}</h3><p>Selecione uma seção. Os recursos ainda indisponíveis estão identificados no Painel e nas Configurações.</p></section>
        {:else if selected && selected !== "general" && selected !== "appearance" && unavailableModules.includes(selected)}
          <section><h3>{TITLES[selected]}</h3><p role="status">Indisponível no Windows nesta versão de validação.</p></section>
        {:else if !selected}
          <Welcome {snapshots} configure={(m) => void focus(m)}
            connectClaude={() => void (core.connectClaude?.() ?? Promise.reject()).catch(() => focus("ai"))} />
        {:else if selected === "general"}
          <GeneralSettings {core} />
        {:else if selected === "appearance"}
          <AppearanceSettings {appearance} save={(a) => core.setAppearance ? core.setAppearance(a) : Promise.reject()} />
        {:else if selected === "ai"}
          <AiSettings data={snapshots.ai?.data as AiData | undefined}
            connect={core.connectClaude ?? unavailable("unwritable")}
            disconnect={core.disconnectClaude ?? unavailable("unwritable")}
            setUsage={core.setClaudeUsage ?? unavailable("unwritable")} />
        {:else if selected === "agents"}
          <AgentSettings setHooks={core.setAgentHooks} data={snapshots.agents?.data as AgentsData | undefined}
            save={(preferences) => core.setAgentNotifications?.(preferences)} />
        {:else if selected === "github"}
          <GithubSettings data={snapshots.github?.data as GithubData | undefined}
            save={core.saveGithubToken ?? unavailable("offline")} />
        {:else if selected === "tasks"}
          <TasksSettings data={snapshots.tasks?.data as TasksData | undefined} generation={snapshots.tasks?.contextGeneration ?? 0} {core} />
        {:else}
          <NetworkSettings data={snapshots.network?.data as NetworkData | undefined}
            setAutomaticSpeed={(enabled) => core.setAutomaticSpeed?.(enabled)} />
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .settings {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    gap: 18px;
    overflow-y: auto;
    padding: 6px 12px 24px 30px;
    background: var(--bg);
    user-select: text;
  }
  .settings:focus { outline: none; }
  .settings-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 8px 16px 16px;
    border-bottom: 5px solid var(--band-lavender);
  }
  .settings-head h2 {
    font-size: 22px;
    font-weight: 500;
    letter-spacing: 2px;
    text-transform: uppercase;
    color: var(--band-amber-text);
  }

  .body {
    display: grid;
    grid-template-columns: 168px minmax(0, 1fr);
    align-items: start;
    gap: 22px;
  }
  nav {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .settings .group {
    margin: 10px 0 2px;
    padding: 0 12px;
    font-size: 11px;
    letter-spacing: 1px;
    text-transform: uppercase;
    color: var(--muted);
  }
  .settings .group:first-child { margin-top: 0; }
  nav ul { list-style: none; display: flex; flex-direction: column; gap: 2px; }
  .settings nav button {
    width: 100%;
    margin: 0;
    min-height: 32px;
    padding: 6px 12px;
    border: none;
    border-left: 3px solid transparent;
    border-radius: 0;
    background: none;
    color: var(--muted);
    text-align: left;
    white-space: nowrap;
  }
  .settings nav button:hover { color: var(--text); }
  .mark {
    margin-left: 6px;
    font: 600 12px var(--mono);
    color: var(--warn);
  }
  .settings nav button[aria-current] {
    border-left-color: var(--accent);
    background: var(--surface-2);
    color: var(--text);
  }

  .content { display: flex; flex-direction: column; gap: 18px; min-width: 0; }
  .pane {
    --card-band: var(--band-cyan);
    background: var(--module-bg);
    border-top: 5px solid var(--card-band);
  }
  .pane :global(h3:focus) { outline: none; }
  .general, .welcome { --card-band: var(--band-amber); }
  .appearance { --card-band: var(--band-lavender); }
  .agents { --card-band: var(--band-violet); }
  .tasks { --card-band: var(--band-amber); }
  .github { --card-band: var(--band-steel); }
  .network { --card-band: var(--band-blue); }

  /* Shared form language for every settings section, including module ones. */
  .settings :global(section) { padding: 18px 20px 20px; font-size: 13px; line-height: 1.45; }
  .settings :global(h3) {
    margin: 0 0 12px;
    font-size: 16px;
    font-weight: 500;
    letter-spacing: 1px;
    text-transform: uppercase;
    color: var(--module-title, var(--card-band, var(--band-cyan)));
  }
  .settings :global(.welcome h3) { color: var(--band-amber-text); }
  .settings :global(h4) { margin: 18px 0 6px; font-size: 13px; font-weight: 600; color: var(--text); }
  .settings :global(h3 + h4) { margin-top: 0; }
  /* A section split into blocks, one per provider: flat, separated by a rule. */
  .settings :global(section section) { padding: 0; }
  .settings :global(section section > h4) { margin-top: 0; }
  .settings :global(section section + section) { margin-top: 18px; padding-top: 14px; border-top: 1px solid var(--line); }
  .settings :global(h5) { margin: 16px 0 4px; font-size: 13px; font-weight: 600; color: var(--text); }
  .settings :global(p) { margin: 8px 0; }
  .settings :global(label) { display: block; margin: 12px 0 6px; color: var(--muted); }
  .settings :global(input:not([type="checkbox"]):not([type="radio"])) {
    width: 100%;
    min-height: 36px;
    padding: 8px 10px;
    font: 13px var(--mono);
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--control-line);
    border-radius: 6px;
  }
  .settings :global(input[aria-invalid="true"]) { border-color: var(--bad); }
  .settings :global(input[type="radio"]) { width: 18px; height: 18px; accent-color: var(--accent); }
  .settings :global(button) {
    display: inline-flex;
    align-items: center;
    min-height: 36px;
    margin: 8px 8px 0 0;
    padding: 8px 16px;
    border: 1px solid var(--control-line);
    border-radius: 0 18px 18px 0;
    background: var(--surface-2);
    color: var(--text);
  }
  .settings :global(button[type="submit"]) { background: var(--band-cyan); border-color: var(--band-cyan); color: var(--on-band); font-weight: 600; }
  .settings :global(button:hover:not(:disabled)) { border-color: var(--accent); }
  .settings :global(button:disabled) { opacity: 0.5; cursor: default; }
  .settings :global(:is(button, input):focus-visible) { outline: 2px solid var(--accent); outline-offset: 2px; }
  .settings :global(.warning) { margin: 8px 0; padding: 12px; border: 1px solid var(--warn); }
  .settings :global(.warning b) { color: var(--warn); }
  .settings .back { border-radius: 18px; }

  @media (max-width: 650px) {
    .settings { padding: 4px 4px 24px 8px; }
    .body { grid-template-columns: minmax(0, 1fr); gap: 12px; }
    nav {
      position: static;
      flex-direction: row;
      align-items: center;
      overflow-x: auto;
      overscroll-behavior-x: contain;
    }
    .settings .group { margin: 0; padding: 0 4px 0 8px; }
    nav ul { flex-direction: row; }
    .settings nav button {
      width: auto;
      border-left: none;
      border-bottom: 3px solid transparent;
    }
    .settings nav button[aria-current] { border-bottom-color: var(--accent); }
  }
</style>
