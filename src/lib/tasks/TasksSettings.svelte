<script lang="ts">
  import type { Core, JiraData, TasksData, TaskProvider, TrelloData } from "../core";
  import TrelloSettings from "../trello/TrelloSettings.svelte";
  import JiraSettings from "./JiraSettings.svelte";
  import { taskError } from "./errors";
  let { data, generation, core }: { data?: TasksData; generation: number; core: Core } = $props();
  const id = $props.id();
  const provider = $derived(data?.provider ?? "trello");
  const context = $derived(data?.contextGeneration ?? generation);
  const pending = $derived(data?.state === "cleanupPending");
  const blocked = $derived(data?.state === "settingsUnavailable");
  let busy = $state(false);
  let message = $state("");
  async function run<T>(operation: () => Promise<T>): Promise<T> {
    busy = true; message = "";
    try { return await operation(); } catch (error) { message = taskError(error); throw error; } finally { busy = false; }
  }
  async function choose(event: Event) {
    const select = event.currentTarget as HTMLSelectElement;
    const next = select.value as TaskProvider;
    select.value = provider;
    if (next === provider) return;
    try { await run(() => core.switchTaskProvider?.(context, next) ?? Promise.reject("settingsUnavailable")); } catch { /* The safe explanation is rendered below. */ }
  }
  async function retry() { try { await run(() => core.retryTaskCleanup?.(context) ?? Promise.reject("cleanupFailed")); } catch { /* Keep the pending transaction visible. */ } }
  function connectTrello(key: string, token: string) { return run(() => core.connectTasks ? core.connectTasks(context, { provider: "trello", key, token }) : (core.connectTrello?.(key, token) ?? Promise.reject("offline"))); }
  function selectBoard(board: string) { return run(() => core.selectTaskBoard ? core.selectTaskBoard(context, board) : (core.selectTrelloBoard?.(board) ?? Promise.reject("offline"))); }
  function disconnect() { return run(() => core.disconnectTasks ? core.disconnectTasks(context) : (core.disconnectTrello?.() ?? Promise.reject("keyringUnavailable"))); }
</script>

<section aria-labelledby="{id}-title">
  <h3 id="{id}-title">Tarefas</h3>
  <label class="provider">Provedor de tarefas
    <select value={provider} disabled={busy || data?.writeBusy || pending || blocked} onchange={choose}>
      <option value="trello">Trello</option><option value="jira">Jira</option>
    </select>
  </label>
  <p class="hint">Ao trocar, sua conexão e seu quadro ficam preservados no cofre. A leitura da fonte anterior é limpa.</p>
  {#if message}<p role="alert">{message}</p>{/if}
  {#if busy && !pending}<p role="status">Concluindo alteração…</p>{/if}
  {#if data?.writeBusy}<p role="status">Uma alteração no serviço está em andamento. Aguarde a resposta ou o fim da espera para trocar o contexto.</p>{/if}
  {#if pending}
    <p role="status">{taskError("cleanupFailed")}</p><button disabled={busy} onclick={retry}>{busy ? "Concluindo…" : "Tentar concluir a limpeza"}</button>
  {:else if blocked}
    <p role="alert">{taskError("settingsUnavailable")}</p>
  {:else if data?.initializing}
    <p role="status">Verificando a conexão preservada…</p>
  {:else}
    {#key `${provider}:${context}`}
      <fieldset disabled={busy || data?.writeBusy}>
        {#if provider === "trello"}
          <TrelloSettings data={data as TrelloData | undefined} connect={connectTrello} {selectBoard} {disconnect}
            openAdmin={() => core.openTrelloAdmin?.()} authorize={(key) => { if (core.authorizeTrello) core.authorizeTrello(context, key); else core.openTrelloAuthorization?.(key); }} />
        {:else}
          <JiraSettings data={data as JiraData | undefined} {selectBoard} {disconnect}
            connect={(site, email, token) => run(() => core.connectTasks?.(context, { provider: "jira", site, email, token }) ?? Promise.reject("offline"))} />
        {/if}
      </fieldset>
    {/key}
  {/if}
</section>

<style>
  h3 { margin-top:0; }
  .provider { display:flex; gap:12px; align-items:center; flex-wrap:wrap; }
  select { padding:6px 10px; color:var(--text); background:var(--surface); border:1px solid var(--line); border-radius:4px; }
  fieldset { border:0; margin:18px 0 0; padding:0; min-width:0; }
  .hint { color:var(--muted); font-size:12px; line-height:1.5; }
  [role="alert"] { color:var(--warn); }
</style>
