<script lang="ts">
  import type { JiraData } from "../core";
  import { taskError } from "./errors";
  let { data, connect, selectBoard, disconnect }: { data?: JiraData; connect:(site:string,email:string,token:string)=>Promise<string>; selectBoard:(board:string)=>Promise<void>; disconnect:()=>Promise<void> } = $props();
  const id = $props.id();
  let site = $state(""); let email = $state(""); let token = $state("");
  let busy = $state(false); let message = $state(""); let editing = $state(false);
  const needsCredential = $derived(editing || data?.state === "missingCredential" || data?.state === "invalidCredential");
  async function submit(event: SubmitEvent) {
    event.preventDefault(); if (busy) return;
    busy = true; message = ""; const candidate = token; token = "";
    try { await connect(site, email, candidate); editing = false; } catch (error) { message = taskError(error); } finally { busy = false; }
  }
  async function choose(event: Event) {
    const select = event.currentTarget as HTMLSelectElement; const board = select.value; select.value = data?.board?.id ?? "";
    if (!board || busy) return;
    busy = true; message = ""; try { await selectBoard(board); } catch (error) { message = taskError(error); } finally { busy = false; }
  }
  async function forget() { busy = true; message = ""; try { await disconnect(); } catch (error) { message = taskError(error); } finally { busy = false; } }
</script>

<section aria-labelledby="{id}-title">
  <h3 id="{id}-title">Jira Cloud</h3>
  {#if data?.account}<p>Conta: {data.account.login}</p>{/if}
  {#if data?.board}<p>Quadro: {data.board.name}</p>{/if}
  {#if data?.state === "invalidCredential"}<p role="alert">{taskError("invalidCredential")}</p>{/if}
  {#if message}<p role="alert">{message}</p>{/if}
  {#if !data}<p role="status">Verificando conexão…</p>{:else if needsCredential}
    <form onsubmit={submit}>
      <label for="{id}-site">Site Jira Cloud</label><input id="{id}-site" type="url" placeholder="https://exemplo.atlassian.net" bind:value={site} required autocomplete="off" />
      <label for="{id}-email">E-mail da conta Atlassian</label><input id="{id}-email" type="email" bind:value={email} required autocomplete="off" />
      <label for="{id}-token">Token de API do Jira</label><input id="{id}-token" type="password" bind:value={token} required autocomplete="new-password" />
      <p class="hint">Conexão pessoal por token de API. Os dados ficam no cofre deste sistema.</p>
      <button type="submit" disabled={busy || !site.trim() || !email.trim() || !token.trim()}>{busy ? "Validando…" : "Validar e salvar"}</button>
    </form>
  {:else}
    <label for="{id}-board">Quadro Jira</label>
    <select id="{id}-board" value={data.board?.id ?? ""} onchange={choose} disabled={busy || data.refreshing}>
      <option value="" disabled>Selecione um quadro</option>
      {#each data.boards as board (board.id)}<option value={board.id}>{board.name}</option>{/each}
    </select>
    {#if !data.boards.length}<p>Nenhum quadro acessível foi listado. Confira seu acesso no Jira e atualize.</p>{/if}
    <div class="actions"><button onclick={() => editing = true} disabled={busy}>Atualizar conexão</button><button onclick={forget} disabled={busy}>Desconectar Jira</button></div>
    <p class="hint">Desconectar remove a credencial e o quadro deste aplicativo. O token não é revogado no Jira.</p>
  {/if}
</section>

<style>
  form { display:grid; gap:8px; }
  input, select { box-sizing:border-box; width:100%; min-width:0; padding:8px; background:var(--surface); color:var(--text); border:1px solid var(--line); border-radius:4px; }
  form button { justify-self:start; }
  .hint { color:var(--muted); font-size:12px; line-height:1.5; }
  .actions { display:flex; gap:10px; margin-top:16px; flex-wrap:wrap; }
  [role="alert"] { color:var(--warn); }
</style>
