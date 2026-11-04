<script lang="ts">
  import { onDestroy } from "svelte";
  import type { Core, JiraData, TasksData, TrelloData } from "../core";
  import TrelloModule from "../trello/TrelloModule.svelte";
  import JiraModule from "./JiraModule.svelte";
  import { taskError } from "./errors";
  let { data, generation, core, configure }: { data: TasksData | null; generation: number; core: Core; configure: () => void } = $props();
  const provider = $derived(data?.provider ?? "trello");
  const context = $derived(data?.contextGeneration ?? generation);
  const unavailable = $derived(data?.state === "cleanupPending" || data?.state === "settingsUnavailable");
  const content = $derived(!unavailable ? data as TrelloData | JiraData | null : null);
  let cooldown = $state(false);
  let timer: ReturnType<typeof setTimeout>;
  onDestroy(() => clearTimeout(timer));
  function refresh() {
    if (cooldown) return;
    cooldown = true; timer = setTimeout(() => cooldown = false, 10_000);
    if (core.refreshTasks) core.refreshTasks(context); else core.refreshTrello?.();
  }
  function open(url: string) { if (core.openTask) core.openTask(context, url); else core.openTrelloCard?.(url); }
  function move(item: string, list: string) { return core.performTaskAction ? core.performTaskAction(context, { kind: "move", item, list }) : (core.moveTrelloCard?.(item, list) ?? Promise.reject("readRequired")); }
</script>

<header>
  <div><h2 id="module-tasks-title">Tarefas</h2><span class="source">{provider === "jira" ? "Jira" : "Trello"}</span></div>
  <div class="actions">
    <button aria-label="Atualizar Tarefas" title="Atualizar Tarefas" disabled={unavailable || data?.initializing || content?.refreshing || cooldown} onclick={refresh}>↻</button>
    <button aria-label="Configurar Tarefas" title="Configurar Tarefas" onclick={configure}>⚙</button>
  </div>
</header>
{#if unavailable}
  <p role="alert">{taskError(data?.state === "cleanupPending" ? "cleanupFailed" : "settingsUnavailable")}</p>
  <button onclick={configure}>Abrir Configurações de Tarefas</button>
{:else if data?.initializing}
  <p role="status">Verificando conexão com {provider === "jira" ? "Jira" : "Trello"}…</p>
{:else}
  {#key `${provider}:${context}`}
    {#if provider === "jira"}
      <JiraModule data={content as JiraData | null} {core} generation={context} canWrite={data?.canWrite ?? false} {configure} />
    {:else}
      <TrelloModule data={content as TrelloData | null} {refresh} {configure} openCard={open} moveCard={move} embedded canWrite={data?.canWrite ?? true} />
    {/if}
  {/key}
{/if}

<style>
  header { display:flex; align-items:center; justify-content:space-between; gap:12px; margin-bottom:12px; }
  header > div { display:flex; align-items:center; gap:10px; }
  h2 { margin:0; font-size:15px; }
  .source { color:var(--muted); font-size:11px; border:1px solid var(--line); border-radius:4px; padding:2px 5px; }
  .actions button { font-size:17px; min-width:28px; min-height:28px; }
  p { color:var(--muted); line-height:1.5; }
</style>
