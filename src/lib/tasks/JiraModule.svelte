<script lang="ts">
  import { untrack } from "svelte";
  import type { Core, JiraData, JiraIssue, TaskTransition } from "../core";
  import { taskError } from "./errors";
  let { data, core, generation, canWrite, configure }: { data:JiraData|null; core:Core; generation:number; canWrite:boolean; configure:()=>void } = $props();
  let dialog: HTMLDialogElement;
  let opener: HTMLElement | null = null;
  let selected = $state<string | null>(null);
  let transitions = $state<TaskTransition[]>([]);
  let transition = $state("");
  let busy = $state(false);
  let localReadRequired = $state(false);
  let message = $state("");
  let request = 0;
  let previous: JiraData | null;
  const item = $derived(data?.issues.find(i => i.id === selected));
  const chosen = $derived(transitions.find(t => t.id === transition));
  const writable = $derived(canWrite && !localReadRequired && !data?.paused && !data?.requiresRead && !data?.rateLimited);
  $effect(() => {
    const current = data;
    untrack(() => {
      if (previous === current) return;
      previous = current; request++; transitions = []; transition = "";
      if (canWrite && current?.state === "ready" && !current.requiresRead) localReadRequired = false;
      if (selected && !current?.issues.some(i => i.id === selected)) selected = null;
    });
  });
  function openBoard(event: MouseEvent) { opener = event.currentTarget as HTMLElement; dialog.showModal(); }
  function close() { request++; dialog.close(); opener?.focus(); }
  function escape(event: KeyboardEvent) { if (event.key === "Escape" && dialog?.open) { event.preventDefault(); event.stopPropagation(); close(); } }
  function openItem(issue: JiraIssue) { core.openTask?.(generation, issue.url); }
  async function chooseItem(issue: JiraIssue) { request++; selected = issue.id; transitions = []; transition = ""; message = ""; if (writable) await loadTransitions(issue); }
  async function loadTransitions(issue = item) {
    if (!issue || !writable || busy) return;
    const ticket = ++request; const snapshot = data; busy = true; message = "";
    try {
      const options = await (core.taskTransitions?.(generation, issue.id) ?? Promise.reject("readRequired"));
      if (ticket === request && data === snapshot && selected === issue.id) { transitions = options; if (!options.length) message = "Não há transições permitidas para este item."; }
    } catch (error) { if (ticket === request) message = taskError(error); }
    finally { busy = false; }
  }
  async function apply() {
    if (!item || !chosen || !writable || busy) return;
    if (chosen.requiresFields) { openItem(item); return; }
    const ticket = ++request; busy = true; localReadRequired = true; message = "";
    try {
      await (core.performTaskAction?.(generation, { kind:"transition", item:item.id, transition:chosen.id }) ?? Promise.reject("readRequired"));
      if (ticket === request) { message = "Transição confirmada. Atualizando o quadro…"; transitions = []; transition = ""; }
    } catch (error) { if (ticket === request) message = taskError(error); }
    finally { busy = false; }
  }
</script>

<svelte:window onkeydowncapture={escape} />

{#if !data}<p role="status">Aguardando leitura do Jira…</p>
{:else if data.state === "missingCredential" || data.state === "invalidCredential"}
  <p>{data.state === "missingCredential" ? "Jira não conectado" : taskError("invalidCredential")}</p><button onclick={configure}>Configurar Jira</button>
{:else if data.state === "chooseBoard"}
  <p>Selecione um quadro Jira nas Configurações.</p><button onclick={configure}>Escolher quadro Jira</button>
{:else}
  {#if data.refreshing}<p role="status">Atualizando Jira…</p>{/if}
  {#if data.paused}<p role="status">Sem conexão. Exibindo a última leitura em memória.</p>{/if}
  {#if data.state === "error"}<p role="alert">{taskError(data.reason ?? "offline")}</p>{/if}
  {#if data.lastSuccessAt != null && (data.paused || data.state !== "ready")}<p class="muted">Última leitura: {new Date(data.lastSuccessAt).toLocaleString("pt-BR")}</p>{/if}
  {#if data.board}
    <strong>{data.board.name}</strong><p>{data.issues.length} itens · {data.columns.length} colunas</p>
    {#if !data.issues.length}<p>Quadro sem itens acessíveis.</p>{/if}
    <ul class="preview">{#each data.issues.slice(0,3) as issue (issue.id)}<li><span>{issue.key}</span> {issue.title}</li>{/each}</ul>
    <button onclick={openBoard}>Abrir quadro Jira</button>
  {:else}<p>Ainda não há leitura disponível. Atualize o quadro.</p>{/if}
{/if}

<dialog bind:this={dialog} aria-labelledby="jira-board-title" oncancel={(event) => { event.preventDefault(); close(); }}>
  <header><h3 id="jira-board-title">{data?.board?.name ?? "Quadro Jira"}</h3><button aria-label="Fechar quadro Jira" onclick={close}>×</button></header>
  {#if data?.paused}<p role="status">Última leitura em memória. As alterações estão indisponíveis sem uma atualização válida.</p>{/if}
  <div class="columns">
    {#each data?.columns ?? [] as column (column.id)}
      <section aria-labelledby="jira-column-{column.id}">
        <h4 id="jira-column-{column.id}">{column.name}</h4>
        {#each data?.issues.filter(issue => column.statuses.includes(issue.status.id)) ?? [] as issue (issue.id)}
          <button class="issue" class:selected={selected === issue.id} aria-label={`${issue.key} ${issue.title} ${issue.status.name}`} aria-pressed={selected === issue.id} onclick={() => chooseItem(issue)}><span>{issue.key}</span><strong>{issue.title}</strong><small>{issue.status.name}</small></button>
        {:else}<p class="muted">Sem itens</p>{/each}
      </section>
    {/each}
  </div>
  {#if item}
    <section class="details" aria-label="Detalhes do item Jira">
      <h4>{item.key} · {item.title}</h4>
      <dl>
        {#if item.type}<div><dt>Tipo</dt><dd>{item.type}</dd></div>{/if}
        <div><dt>Estado</dt><dd>{item.status.name}</dd></div>
        {#if item.priority}<div><dt>Prioridade</dt><dd>{item.priority}</dd></div>{/if}
        {#if item.assignee}<div><dt>Responsável</dt><dd>{item.assignee}</dd></div>{/if}
        {#if item.due}<div><dt>Prazo</dt><dd>{item.due.split("-").reverse().join("/")}</dd></div>{/if}
      </dl>
      <button onclick={() => openItem(item)}>Abrir item no Jira</button>
      {#if !writable}<p>Atualize o quadro antes de alterar o estado.</p>
      {:else if transitions.length}
        <label>Transição permitida <select bind:value={transition} disabled={busy}><option value="">Escolha uma transição</option>{#each transitions as option (option.id)}<option value={option.id}>{option.name} → {option.status}</option>{/each}</select></label>
        {#if chosen?.requiresFields}<p>{taskError("requiredFields")}</p>{/if}
        <button disabled={busy || !chosen} onclick={apply}>{busy ? "Aguardando resposta…" : chosen?.requiresFields ? "Continuar no Jira" : "Confirmar transição"}</button>
      {:else}<button onclick={() => loadTransitions()} disabled={busy}>{busy ? "Consultando…" : "Consultar transições permitidas"}</button>{/if}
    </section>
  {/if}
  {#if message}<p role="status">{message}</p>{/if}
  {#if data?.movement?.state === "unconfirmed"}<p role="alert">{taskError("unconfirmed")}</p>{/if}
</dialog>

<style>
  p { line-height:1.5; }
  .muted, small { color:var(--muted); font-size:12px; }
  .preview { list-style:none; padding:0; font-size:12px; }
  .preview li { overflow:hidden; white-space:nowrap; text-overflow:ellipsis; padding:4px 0; }
  .preview span { color:var(--muted); }
  dialog { width:min(1100px,calc(100vw - 64px)); max-height:85vh; padding:22px; border:1px solid var(--line); border-radius:8px; background:var(--surface); color:var(--text); }
  dialog::backdrop { background:rgb(0 0 0 / 55%); }
  header { display:flex; justify-content:space-between; align-items:center; gap:16px; }
  h3 { margin:0; }
  header button { font-size:24px; min-width:32px; }
  .columns { display:flex; gap:12px; overflow-x:auto; margin:18px 0; padding-bottom:10px; }
  .columns section { flex:0 0 210px; background:var(--surface-2); border:1px solid var(--line); border-radius:5px; padding:10px; }
  h4 { margin:0 0 10px; }
  .issue { display:grid; gap:6px; width:100%; margin:6px 0; padding:10px; text-align:left; color:var(--text); background:var(--surface); border:1px solid var(--line); border-radius:4px; }
  .issue span { font:11px monospace; color:var(--muted); }
  .issue.selected { border-color:var(--accent); }
  .details { border-top:1px solid var(--line); padding-top:14px; }
  dl { display:flex; gap:12px 24px; flex-wrap:wrap; }
  dt { font-size:11px; color:var(--muted); } dd { margin:4px 0; }
  select { margin:10px; max-width:100%; padding:6px; background:var(--surface); color:var(--text); border:1px solid var(--line); }
</style>
