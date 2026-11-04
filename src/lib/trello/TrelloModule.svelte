<script lang="ts">
  /** The Trello summary and full board as prototype B shows them; states B does not show become body lines. */
  import { onMount, tick, untrack } from "svelte";
  import type { TrelloData } from "../core";
  import { t, interpolate } from "../strings";
  import AbsentArea, { type Absence } from "../AbsentArea.svelte";
  let { data, refresh, configure, openCard, moveCard, embedded = false, canWrite = true }: {
    data: TrelloData | null; refresh: () => void; configure: () => void; openCard: (url: string) => void;
    /** Rejects when the core could not move the card. */
    moveCard: (card: string, list: string) => Promise<void>;
    embedded?: boolean; canWrite?: boolean;
  } = $props();
  let now = $state(Date.now());
  /** Manual refreshes stay at least ten seconds apart. */
  let allowedAt = $state(0);
  let tipsDismissed = $state(false);
  let titleActions = $state<HTMLDivElement>();
  onMount(() => {
    const timer = setInterval(() => now = Date.now(), 1000);
    function dismissDescription(event: KeyboardEvent) {
      if (event.key !== "Escape" || tipsDismissed || !titleActions ||
          !(titleActions.contains(document.activeElement) || titleActions.matches(":hover"))) return;
      tipsDismissed = true;
      event.preventDefault();
      event.stopPropagation();
    }
    window.addEventListener("keydown", dismissDescription, true);
    return () => { clearInterval(timer); window.removeEventListener("keydown", dismissDescription, true); };
  });
  const relative = new Intl.RelativeTimeFormat("pt-BR", { numeric: "auto" });
  function age(when: number) {
    const seconds = Math.max(0, Math.floor((now - when) / 1000));
    if (seconds < 60) return relative.format(0, "second");
    if (seconds < 3600) return relative.format(-Math.floor(seconds / 60), "minute");
    if (seconds < 86400) return relative.format(-Math.floor(seconds / 3600), "hour");
    return relative.format(-Math.floor(seconds / 86400), "day");
  }
  const date = new Intl.DateTimeFormat("pt-BR", { day: "2-digit", month: "short" });
  // Trello label palette; _dark/_light variants share their base hue.
  const colors: Record<string, string> = {
    green: "#4bce97", yellow: "#f5cd47", orange: "#fea362", red: "#f87168", purple: "#9f8fef",
    blue: "#579dff", sky: "#6cc3e0", lime: "#94c748", pink: "#e774bb", black: "#8590a2",
  };
  const color = (name: string) => colors[name.replace(/_(dark|light)$/, "")] ?? "var(--muted)";
  /** Labels are named in words too, so they never rely on color alone. */
  function colorName(name: string) {
    const [base, shade] = name.split("_") as [string, "dark" | "light" | undefined];
    const word = (t.trello.labelColors as Record<string, string>)[base] ?? base;
    return shade && t.trello.labelShade[shade] ? interpolate(t.trello.labelShade[shade], { color: word }) : word;
  }
  const paused = $derived(data?.paused ?? false);
  const stale = $derived(data?.state === "invalidCredential" && !paused);
  function refreshNow() { allowedAt = Date.now() + 10000; refresh(); }
  /** A previous board shown while the current reading is missing: dimmed under its age. */
  const old = $derived(data?.lists != null && (data.state !== "ready" || paused));
  /** Without a board to show, the body is one area saying why. */
  const absence = $derived.by((): Absence | null => {
    const setup = { label: t.absent.configure, run: configure };
    if (data?.state === "missingCredential")
      return { text: t.trello.missingCredential, tone: "warn", detail: data.pendingKey ? t.trello.pendingToken : undefined, action: setup };
    if (data?.state === "chooseBoard") return { text: t.trello.chooseBoard, tone: "neutral", action: setup };
    if (data?.lists != null) return null;
    if (stale) return { text: t.trello.invalidCredential, tone: "warn", action: { label: t.absent.reconnect, run: configure } };
    if (data?.state === "error" && !paused) return {
      text: data.failureSince != null ? interpolate(t.trello.failureNoReading, { age: age(data.failureSince) }) : t.trello.error,
      tone: "warn",
      action: { label: t.absent.tryAgain, run: refreshNow, disabled: data.refreshing || now < allowedAt },
    };
    return { text: t.trello.noReading, tone: "neutral" };
  });

  let selected = $state<string | null>(null);
  let destination = $state("");
  /** Last move outcome, shown in the board and, after a failure, in the summary. */
  let outcome = $state<{ kind: "moving" | "moved" | "failed" | "unconfirmed"; text: string } | null>(null);
  /** Destination by card until the core answers; a settled move lasts until the next board. */
  let pending = $state<Record<string, { list: string; move: number; settled?: boolean }>>({});
  let moves = 0;
  $effect(() => {
    void data;
    untrack(() => {
      if (outcome?.kind === "unconfirmed" && canWrite && data?.state === "ready" && data.movement?.state !== "unconfirmed") outcome = null;
      for (const card in pending) if (pending[card].settled) delete pending[card];
      // A refresh keeps the selection unless the card left the board.
      if (selected && !listOf(selected)) selected = null;
      // A destination that left the board, or now holds the card, is no longer an option.
      if (destination && (!selected || destination === listOf(selected) || !lists.some((l) => l.id === destination))) destination = "";
    });
  });
  // Optimistic view: a pending card shows at the bottom of its destination list; the summary reads it too.
  const lists = $derived.by(() => {
    const cards = (data?.lists ?? []).flatMap((list) => list.cards);
    return (data?.lists ?? []).map((list) => ({
      ...list,
      cards: [
        ...list.cards.filter((card) => (pending[card.id]?.list ?? list.id) === list.id),
        ...cards.filter((card) => pending[card.id]?.list === list.id && !list.cards.includes(card)),
      ],
    }));
  });
  const total = $derived(lists.reduce((n, list) => n + list.cards.length, 0));
  const title = (card: { name: string }) => card.name || t.trello.hiddenTitle;
  const listOf = (card: string) => lists.find((list) => list.cards.some((c) => c.id === card))?.id;
  const nameOf = (list: string) => lists.find((l) => l.id === list)?.name ?? "";
  const movable = $derived(canWrite && data != null && !paused && data.state !== "invalidCredential");
  const selectedList = $derived(selected ? listOf(selected) : undefined);
  const selectedCard = $derived(lists.flatMap((l) => l.cards).find((c) => c.id === selected));
  const plural = (count: number, one: string, many: string) => count === 1 ? one : interpolate(many, { count: String(count) });
  const cardCount = (count: number) => plural(count, t.trello.cardsOne, t.trello.cardsMany);
  const boardCounts = $derived(interpolate(t.trello.boardCounts, { cards: cardCount(total), lists: plural(lists.length, t.trello.listsOne, t.trello.listsMany) }));

  let dialog: HTMLDialogElement;
  let columns = $state<HTMLElement>();
  const columnEls: Record<string, HTMLElement> = {};
  let detailOpen = $state(false);
  let restoringFocus = false;
  let opener: HTMLElement | null = null;
  let scrollLeft = 0;

  function cardButton(card: string) {
    return dialog?.querySelector<HTMLElement>(`[data-card="${CSS.escape(card)}"]`) ?? null;
  }
  async function openBoard(event: MouseEvent, card?: string) {
    if (dialog.open) return;
    opener = event.currentTarget as HTMLElement;
    if (card) { selected = card; destination = ""; }
    outcome = null;
    dialog.showModal();
    detailOpen = true;
    await tick();
    if (columns) columns.scrollLeft = scrollLeft;
    // Reopening keeps the context: the still valid selection gets the focus back.
    if (selected) {
      columnEls[listOf(selected) ?? ""]?.scrollIntoView({ block: "nearest", inline: "nearest" });
      cardButton(selected)?.focus();
    }
  }
  function closeBoard() {
    if (columns) scrollLeft = columns.scrollLeft;
    // Whatever happened in the board was seen there; a later failure still reaches the summary.
    if (outcome?.kind !== "moving") outcome = null;
    restoringFocus = true;
    dialog.close();
    detailOpen = false;
    opener?.focus({ preventScroll: true });
    restoringFocus = false;
  }
  function closeOnEscape(event: KeyboardEvent) {
    if (event.key === "Escape" && dialog?.open) {
      event.preventDefault();
      event.stopPropagation();
      closeBoard();
    }
  }

  function select(card: string) {
    selected = selected === card ? null : card;
    destination = "";
    if (outcome?.kind !== "moving") outcome = null;
  }
  function openFromCard(card: { id: string; url: string }) {
    // The two clicks of a double click toggled the selection off; the opened card stays selected.
    selected = card.id;
    openCard(card.url);
  }
  async function cancelSelection() {
    const card = selected;
    selected = null;
    destination = "";
    await tick();
    if (card) cardButton(card)?.focus();
  }
  /** Moves re-create the card element in another column: focus goes with the card. */
  async function focusMovedCard(card: string) {
    await tick();
    if (detailOpen) cardButton(card)?.focus();
  }
  /** After a revert, refocus the card only if its element held the focus, never stealing it. */
  async function refocusIfLost(card: string) {
    await tick();
    const lost = !dialog.contains(document.activeElement) || document.activeElement === dialog;
    if (detailOpen && lost) cardButton(card)?.focus();
  }
  function moveTo(list: string, card = selected) {
    if (!card || !movable) return;
    const origin = listOf(card);
    if (!origin || origin === list) return;
    const name = title(lists.flatMap((l) => l.cards).find((c) => c.id === card)!);
    const target = nameOf(list), originName = nameOf(origin);
    selected = card;
    destination = "";
    const move = ++moves;
    pending[card] = { list, move };
    outcome = { kind: "moving", text: interpolate(t.trello.moving, { card: name, list: target }) };
    void focusMovedCard(card);
    columnEls[list]?.scrollIntoView({ block: "nearest", inline: "nearest" });
    // A later move of the same card owns the overlay.
    const current = () => pending[card]?.move === move;
    moveCard(card, list).then(
      () => {
        if (!current()) return;
        pending[card].settled = true;
        outcome = { kind: "moved", text: interpolate(t.trello.moved, { card: name, list: target }) };
      },
      (error) => {
        if (!current()) return;
        delete pending[card];
        outcome = error === "unconfirmed"
          ? { kind: "unconfirmed", text: "Resultado não confirmado. Atualize o quadro antes de tentar novamente." }
          : { kind: "failed", text: interpolate(t.trello.moveFailedDetail, { card: name, list: originName }) };
        columnEls[origin]?.scrollIntoView({ block: "nearest", inline: "nearest" });
        void refocusIfLost(card);
      },
    );
  }

  let dragged = $state<string | null>(null);
  function dragStart(event: DragEvent, card: string) {
    if (!movable) { event.preventDefault(); return; }
    dragged = card;
    selected = card;
    destination = "";
    // WebKitGTK only starts a drag that carries data: only the opaque card id, never its content.
    event.dataTransfer?.setData("text/plain", card);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }
  function dragOver(event: DragEvent, list: string) {
    if (!dragged || listOf(dragged) === list) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  }
  function drop(event: DragEvent, list: string) {
    if (!dragged) return;
    event.preventDefault();
    const card = dragged;
    dragged = null;
    moveTo(list, card);
  }
</script>

<svelte:window onkeydowncapture={closeOnEscape} />

{#snippet labels(card: { labels: string[] })}
  {#if card.labels.length}
    {@const named = interpolate(t.trello.labels, { colors: card.labels.map(colorName).join(", ") })}
    <span class="labels" role="img" aria-label={named} title={named}>
      {#each card.labels as label}<i data-testid="label" style:background={color(label)}></i>{/each}
    </span>
  {/if}
{/snippet}
{#snippet due(card: { due: number | null; overdue: boolean; dueComplete: boolean })}
  {#if card.due != null}
    <small class="due num" class:overdue={card.overdue} class:done={card.dueComplete}>
      <span aria-hidden="true">{card.overdue ? "⚠ " : card.dueComplete ? "✓ " : ""}</span>{interpolate(card.overdue ? t.trello.overdue : card.dueComplete ? t.trello.dueDone : t.trello.due, { date: date.format(card.due) })}
    </small>
  {:else}
    <small class="due">{t.trello.noDue}</small>
  {/if}
{/snippet}

{#if !embedded}<header>
  <h2 id="module-trello-title">{t.modules.trello}</h2>
  <div class="actions" bind:this={titleActions}>
    <span class="action-wrap" role="group" onpointerenter={() => tipsDismissed = false} onfocusin={() => tipsDismissed = false}>
      <button class="icon" aria-label={t.trello.refresh} aria-describedby="trello-refresh-description"
        disabled={paused || data?.refreshing || now < allowedAt} onclick={refreshNow}><span aria-hidden="true">↻</span></button>
      <span class="tip" hidden={tipsDismissed} id="trello-refresh-description" role="tooltip">{t.trello.refreshDescription}</span>
    </span>
    <span class="action-wrap" role="group" onpointerenter={() => tipsDismissed = false} onfocusin={() => tipsDismissed = false}>
      <button class="icon" aria-label={t.trello.configureTrello} aria-describedby="trello-configure-description" onclick={configure}><span aria-hidden="true">⚙</span></button>
      <span class="tip" hidden={tipsDismissed} id="trello-configure-description" role="tooltip">{t.trello.configureDescription}</span>
    </span>
  </div>
</header>{/if}

<div class="trello" onfocusin={(event) => { if (restoringFocus) event.stopPropagation(); }}>
  {#if data?.refreshing}<small role="status" class="line">{t.trello.refreshing}</small>{/if}
  {#if absence}
    <AbsentArea {absence} />
  {:else if data}
    {#if stale}
      <small class="line alert"><span aria-hidden="true">{"⚠ "}</span>{t.trello.invalidCredential}{" · "}<button class="link" onclick={configure}>{t.trello.reconnect}</button></small>
    {/if}
    {#if data.state === "error" && !paused}
      <small class="line alert"><span aria-hidden="true">{"⚠ "}</span>{data.failureSince != null ? interpolate(t.trello.failureAge, { age: age(data.failureSince) }) : t.trello.error}</small>
    {/if}
    {#if data.lastSuccessAt != null && (data.state !== "ready" || paused)}
      <small class="line">{interpolate(t.trello.updatedAge, { age: age(data.lastSuccessAt) })}</small>
    {/if}
    {#if (outcome?.kind === "failed" || outcome?.kind === "unconfirmed") && !detailOpen}<small role="alert" class="line alert">{outcome.text}</small>{/if}
    <div class="summary" data-stale={old}>
      {#if data.board}
        <div class="board-summary"><b>{data.board.name}</b><span>{boardCounts}</span></div>
      {/if}
      {#if lists.length === 0}<p class="empty-board">{t.trello.noLists}</p>{:else}
        <ul class="list-counts" aria-label={t.trello.countsByList}>
          {#each lists as list (list.id)}<li><span class="list-name" title={list.name}>{list.name}</span><b class="num">{list.cards.length}</b></li>{/each}
        </ul>
        {#if total === 0}<p class="empty-board">{t.trello.noCards}</p>{/if}
        <button class="detail-button" onclick={(event) => openBoard(event)}>{t.trello.openBoard}<span aria-hidden="true">{" ↗"}</span></button>
      {/if}
    </div>
  {/if}

  <dialog bind:this={dialog} aria-labelledby="trello-board" oncancel={(event) => { event.preventDefault(); closeBoard(); }}>
    <header><h2 id="trello-board">{t.trello.boardDetail}</h2>
      <button class="detail-button" onclick={closeBoard}>{t.trello.close}<span aria-hidden="true">{" ×"}</span></button>
    </header>
    {#if detailOpen}
    <div class="board-toolbar">
      {#if data?.board}<div><h3>{data.board.name}</h3><p class="num">{boardCounts}</p></div>{/if}
      {#if movable}<p>{t.trello.boardHint}</p>{/if}
    </div>
    <div class="move-toolbar">
      {#if selectedCard && selectedList}
        <div class="selection"><b>{title(selectedCard)}</b><small>{interpolate(t.trello.selectedIn, { list: nameOf(selectedList) })}</small></div>
        {#if movable}
          <label>{t.trello.moveSelect}
            <select bind:value={destination}>
              <option value="">{t.trello.chooseList}</option>
              {#each lists.filter((list) => list.id !== selectedList) as list (list.id)}<option value={list.id}>{list.name}</option>{/each}
            </select>
          </label>
          <button class="primary" disabled={!destination} onclick={() => moveTo(destination)}>{t.trello.moveCard}</button>
        {/if}
        <button onclick={cancelSelection}>{t.trello.cancelSelection}</button>
      {:else}
        <p>{movable ? t.trello.selectHint : t.trello.moveUnavailable}</p>
      {/if}
    </div>
    {#if outcome?.kind === "failed" || outcome?.kind === "unconfirmed"}
      <p role="alert" class="move-status failed">{outcome.text}</p>
    {:else}
      <p role="status" class="move-status">{outcome?.text ?? t.trello.noChange}</p>
    {/if}
    {#if lists.length === 0}<p class="empty">{t.trello.noLists}</p>{:else}
      <div class="board-columns" role="group" aria-label={t.trello.boardLists} bind:this={columns} data-stale={stale}>
        {#each lists as list (list.id)}
          {@const target = movable && selectedList != null && selectedList !== list.id}
          <!-- Dropping is a pointer shortcut; "Mover para esta lista" and the toolbar are the keyboard path. -->
          <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
          <section class="board-column" aria-label={list.name} class:target bind:this={columnEls[list.id]}
            ondragover={(event) => dragOver(event, list.id)} ondrop={(event) => drop(event, list.id)}>
            <header><h4>{list.name}</h4><span class="num" aria-hidden="true">{list.cards.length}</span><span class="sr-only">{cardCount(list.cards.length)}</span></header>
            {#if target}<button class="move-here" onclick={() => moveTo(list.id)}><span aria-hidden="true">→ </span>{t.trello.moveHere}</button>{/if}
            <ul>
              {#each list.cards as card (card.id)}
                <li>
                  <button class="board-card" class:selected={selected === card.id} aria-pressed={selected === card.id} data-card={card.id}
                    draggable={movable} ondragstart={(event) => dragStart(event, card.id)} ondragend={() => dragged = null}
                    onclick={() => select(card.id)} ondblclick={() => openFromCard(card)}>
                    <b class:secondary={!card.name}>{title(card)}</b>
                    {@render labels(card)}
                    {@render due(card)}
                  </button>
                  {#if selected === card.id}
                    <button class="open-card" aria-label={interpolate(t.trello.open, { card: title(card) })} onclick={() => openCard(card.url)}>{t.trello.openShort}<span aria-hidden="true">{" ↗"}</span></button>
                  {/if}
                </li>
              {/each}
              {#if list.cards.length === 0}<li class="empty">{t.trello.emptyList}</li>{/if}
            </ul>
          </section>
        {/each}
      </div>
    {/if}
    {/if}
  </dialog>
</div>
<style>
  header { display: flex; align-items: center; gap: 10px; justify-content: space-between; min-height: 28px; margin-bottom: 6px; }
  h2 { font-size: 1.25em; font-weight: 500; text-transform: uppercase; letter-spacing: 1px; }
  small { font-size: .88em; color: var(--muted); }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  .secondary { color: var(--muted); }
  .actions { display: flex; gap: 5px; }
  .action-wrap { position: relative; display: inline-flex; }
  .icon { width: 28px; height: 28px; border: 1px solid var(--line); border-radius: 50%; font-size: 19px; line-height: 1; display: grid; place-items: center; }
  .icon:hover:not(:disabled) { background: var(--surface-2); }
  .icon:disabled { opacity: .45; cursor: default; }
  .tip { position: absolute; z-index: 4; right: 0; top: 100%; min-width: 150px; background: var(--surface-2); color: var(--text); padding: 9px; border: 1px solid var(--control-line); box-shadow: 0 4px 20px #0006; font-size: 12px; font-weight: 400; visibility: hidden; }
  .action-wrap:hover .tip, .action-wrap:focus-within .tip { visibility: visible; }
  /* The free height of the card goes to the list rows. */
  .trello { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .summary { flex: 1; display: flex; flex-direction: column; }
  .line { display: block; margin-bottom: 4px; overflow-wrap: anywhere; }
  .line.alert { color: var(--warn); }
  .link { color: var(--accent); text-decoration: underline; font-size: inherit; }
  .summary[data-stale="true"] { --text: var(--muted); --accent: var(--muted); color: var(--muted); }
  .board-summary { display: flex; justify-content: space-between; gap: 10px; font-size: .93em; margin: 2px 0 12px; overflow-wrap: anywhere; }
  .board-summary b { font-weight: 600; }
  .board-summary span { color: var(--muted); white-space: nowrap; }
  .list-counts { flex: 1; list-style: none; padding: 0; display: grid; grid-template-columns: 1fr 1fr; grid-auto-rows: 1fr; align-items: end; gap: 4px 16px; }
  .list-counts li { display: flex; justify-content: space-between; gap: 6px; min-width: 0; border-bottom: 1px solid var(--line); padding-bottom: 4px; }
  .list-counts .list-name { min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .labels { display: flex; flex-wrap: wrap; gap: 4px; }
  .detail-button { color: var(--accent); border-bottom: 1px solid var(--line); padding: 7px 0; text-align: left; margin-top: 5px; }
  .empty-board { margin-top: 8px; color: var(--muted); }
  .icon:focus-visible, .link:focus-visible, .detail-button:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }

  /* Full board, as TrelloBoardPrototype: 1040 × 610 on the 1100 × 700 target, bounded by the window. */
  dialog { margin: auto; width: min(1040px, 96vw); height: min(610px, 92vh); max-width: 96vw; max-height: 92vh; overflow: hidden; background: var(--surface); color: var(--text); border: 1px solid var(--control-line); padding: 20px; font-size: 13px; }
  dialog[open] { display: flex; flex-direction: column; }
  dialog::backdrop { background: #000a; }
  dialog button { cursor: pointer; }
  dialog button:focus-visible, select:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .board-toolbar { display: flex; justify-content: space-between; gap: 20px; align-items: center; margin: 10px 0 12px; flex: none; }
  h3 { font-size: 17px; font-weight: 500; overflow-wrap: anywhere; }
  dialog p, dialog small { color: var(--muted); font-size: 12px; }
  .board-toolbar p { margin-top: 5px; }
  .move-toolbar { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; min-height: 66px; padding: 10px 12px; background: var(--surface-2); border-left: 4px solid var(--band-amber); flex: none; }
  .selection { display: grid; gap: 4px; max-width: 240px; overflow-wrap: anywhere; }
  label { display: flex; align-items: center; gap: 8px; font-size: 12px; }
  select { min-height: 34px; max-width: 220px; background: var(--bg); color: var(--text); border: 1px solid var(--control-line); padding: 6px; font: inherit; }
  .move-toolbar button { min-height: 32px; padding: 6px 10px; }
  .primary { background: var(--band-cyan); color: var(--on-band); border-radius: 0 16px 16px 0; }
  .primary:disabled { opacity: .55; cursor: default; }
  .move-status { flex: none; min-height: 36px; padding: 8px 0; overflow-wrap: anywhere; }
  dialog .move-status.failed { color: var(--warn); }
  /* Columns scroll sideways, each list scrolls on its own; no card is hidden to fit. */
  .board-columns { display: flex; gap: 12px; overflow-x: auto; min-height: 0; flex: 1; padding: 3px 3px 12px; align-items: stretch; scrollbar-color: var(--control-line) var(--surface); }
  .board-columns[data-stale="true"] { color: var(--muted); }
  .board-column { flex: 0 0 228px; min-width: 0; min-height: 0; display: flex; flex-direction: column; border: 1px solid var(--line); border-top: 4px solid var(--band-blue); background: var(--bg); border-radius: 14px 0 0 0; padding: 10px; }
  .board-column.target { border-color: var(--accent); }
  .board-column header { display: flex; justify-content: space-between; gap: 12px; padding: 3px 2px 10px; margin: 0; min-height: 0; }
  h4 { font-size: 14px; overflow-wrap: anywhere; }
  .board-column header span { color: var(--muted); }
  .move-here { border: 1px dashed var(--accent); color: var(--accent); margin: 0 0 8px; min-height: 32px; padding: 6px 10px; text-align: left; }
  ul { list-style: none; }
  .board-column ul { overflow-y: auto; min-height: 0; padding: 3px; display: flex; flex-direction: column; gap: 9px; }
  .board-column li { flex-shrink: 0; }
  .board-card { display: flex; flex-direction: column; gap: 9px; width: 100%; padding: 12px; text-align: left; color: inherit; background: var(--surface-2); border: 1px solid var(--line); border-radius: 0 12px 12px 0; user-select: none; }
  .board-card[draggable="true"] { cursor: grab; }
  .board-card.selected { outline: 2px solid var(--accent); outline-offset: 0; }
  .board-card b { font-size: 13px; font-weight: 500; overflow-wrap: anywhere; }
  /* Labels sit on top as in B, after the title in the DOM so the card is named by its title first. */
  .board-card .labels { order: -1; }
  .board-card .labels i { width: 26px; height: 5px; border-radius: 3px; }
  .due.overdue { color: var(--warn); }
  .open-card { color: var(--accent); padding: 6px 4px 0; font-size: 12px; text-align: left; }
  .empty { color: var(--muted); padding: 20px 8px; border: 1px dashed var(--line); font-size: 12px; }
  @media (max-width: 650px) { .board-toolbar { align-items: start; flex-direction: column; gap: 4px; } .board-column { flex-basis: 210px; } .selection { max-width: 100%; } }
</style>
