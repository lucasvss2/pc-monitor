<script lang="ts">
  import { onMount } from "svelte";
  import type { NetworkData, NetworkTraffic } from "../core";
  import { interpolate, t } from "../strings";
  import AbsentArea from "../AbsentArea.svelte";

  /** The network summary as prototype B shows it; states B does not show become body lines. */
  let { data, runSpeed = () => {} }: { data: NetworkData | null; runSpeed?: () => void } = $props();
  let now = $state(Date.now());
  onMount(() => {
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(timer);
  });

  /** The core only weighs a speed test younger than six hours; older ones are shown as old. */
  const STALE_SPEED_MS = 6 * 3600 * 1000;
  const number = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 });
  const clock = new Intl.DateTimeFormat("pt-BR", { hour: "2-digit", minute: "2-digit" });
  const date = new Intl.DateTimeFormat("pt-BR", { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });

  function age(when: number) {
    const seconds = Math.max(0, Math.floor((now - when) / 1000));
    if (seconds < 60) return interpolate(t.network.ageSeconds, { value: String(seconds) });
    if (seconds < 3600) return interpolate(t.network.ageMinutes, { value: String(Math.floor(seconds / 60)) });
    if (seconds < 86400) return interpolate(t.network.ageHours, { value: String(Math.floor(seconds / 3600)) });
    return interpolate(t.network.ageDays, { value: String(Math.floor(seconds / 86400)) });
  }
  /** Without a measurement the module says so explicitly, never 0; units as B writes them. */
  const measured = (v: number | null, unit: "ms" | "%") =>
    v === null ? t.network.noValue : unit === "%" ? `${number.format(v)}%` : `${number.format(v)} ms`;
  /** Traffic in KB/s or MB/s, kept apart from the Mbps of the speed test. */
  const rate = (v: number) => (v >= 1048576 ? `${number.format(v / 1048576)} MB/s` : `${number.format(v / 1024)} KB/s`);

  const connected = $derived(data?.connected ?? false);
  const quality = $derived(data?.quality ? t.network[data.quality] : t.network.unmeasured);
  const security = $derived(data?.advancedUnavailable ? t.network.securityUnavailable
    : data?.security ? t.network[data.security] : t.network.unmeasured);
  const alert = $derived(connected && (data?.security === "warning" || data?.security === "risk") ? data.security : null);

  const speed = $derived(data?.speed ?? null);
  const speedStale = $derived(speed !== null && now - speed.measuredAt >= STALE_SPEED_MS);

  /** Chart as in B: every sample of the last minute spread over the full width, scale from the samples themselves. */
  const SLOTS = 60;
  const W = 300, TOP = 6, BOTTOM = 66;
  const history = $derived(data?.history.slice(-SLOTS) ?? []);
  const scale = $derived(Math.max(1, ...history.flatMap((p) => [p.download, p.upload])));
  const x = (i: number) => (history.length === 1 ? W / 2 : (i / (history.length - 1)) * W);
  const y = (v: number) => BOTTOM - (v / scale) * (BOTTOM - TOP);
  const path = (direction: keyof NetworkTraffic) => history.map((p, i) => `${i ? "L" : "M"}${x(i)},${y(p[direction])}`).join(" ");
  const peak = (direction: keyof NetworkTraffic) => Math.max(...history.map((p) => p[direction]));
  const chartSummary = $derived.by(() => {
    if (history.length === 1) {
      const only = history[0];
      return interpolate(t.network.chartSingle, { down: rate(only.download), up: rate(only.upload) });
    }
    const last = history.at(-1)!;
    return interpolate(t.network.chartSummary, {
      count: String(history.length),
      down: rate(last.download), downPeak: rate(peak("download")),
      up: rate(last.upload), upPeak: rate(peak("upload")),
    });
  });

  /** Up to five apps plus Others, on one local scale shared by every bar. */
  const apps = $derived.by(() => {
    const traffic = data?.processTraffic;
    if (!connected || !traffic) return null;
    const rows = [...traffic.apps.map((a) => ({ ...a, others: false })), { name: t.network.others, ...traffic.others, others: true }];
    return { rows, count: traffic.apps.length, max: Math.max(1, ...rows.flatMap((r) => [r.download, r.upload])) };
  });
  const width = (v: number, max: number) => `${(v / max) * 100}%`;
  const appsLabel = $derived.by(() => {
    if (connected && !data?.processTraffic) return t.network.appsUnmeasured;
    if (!apps || apps.count === 0) return t.network.appsDetail;
    return apps.count === 1 ? t.network.appsCountOne : interpolate(t.network.appsCount, { count: String(apps.count) });
  });

  /** Modal detail listing per-app traffic ("Aplicativos de rede") over the summary; it reads the same snapshot, never a new collection. */
  let dialog: HTMLDialogElement;
  let detailOpen = $state(false);
  let restoringFocus = false;
  let opener: HTMLElement | null = null;
  function openApps(event: MouseEvent) {
    if (dialog.open) return;
    opener = event.currentTarget as HTMLElement;
    dialog.showModal();
    detailOpen = true;
  }
  function closeApps() {
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
      closeApps();
    }
  }
</script>

<svelte:window onkeydowncapture={closeOnEscape} />

<div class="network" onfocusin={(event) => { if (restoringFocus) event.stopPropagation(); }}>
  <header><h2 id="module-network-title">{t.modules.network}</h2></header>
  {#if !data}
    <AbsentArea absence={{ text: t.network.noReading, tone: "neutral" }} />
  {:else}
    {#if connected}
      <p class="network-status">
        <span>{data.linkKind === "other" ? t.network.connected : data.wired ? t.network.wired : t.network.wireless}</span>
        <span>{t.network.qualityShort}: {quality}</span>
        <span>{t.network.securityShort}: {security}</span>
      </p>
    {:else}
      <small class="line alert"><span aria-hidden="true">⊘&nbsp;</span>{data.connectionUnavailable ? t.network.connectionUnavailable : t.network.disconnected}</small>
    {/if}
    {#if connected && data.vpn}<small class="line accent">{t.network.vpn}</small>{/if}
    {#if alert}
      {#each data.reasons.length > 0 ? data.reasons : [t.network[alert]] as reason (reason)}
        <small class="line {alert}"><span aria-hidden="true">⚠&nbsp;</span>{reason}</small>
      {/each}
    {/if}
    {#if data.speedRunning}<small class="line" role={data.speedOperation?.origin === "automatic" ? undefined : "status"}>{t.network.testingSpeed}</small>{/if}
    {#if data.speedOperation?.state === "cancelled" && data.speedOperation.reason && Object.hasOwn(t.network.speedCancelled, data.speedOperation.reason)}
      <small class="line alert">{t.network.speedCancelled[data.speedOperation.reason]}</small>
    {/if}
    {#if data.speedFailed}<small class="line alert" role="status"><span aria-hidden="true">⚠&nbsp;</span>{t.network.speedFailed}</small>{/if}
    {#if connected && !data.speedAvailable && !data.speedRunning}
      <small class="line"><span aria-hidden="true">⊘&nbsp;</span>{t.network.speedUnavailable}</small>
    {/if}
    {#if speed && speedStale}
      <small class="line alert"><span aria-hidden="true">⚠&nbsp;</span>{interpolate(t.network.speedStale, { age: age(speed.measuredAt), date: date.format(speed.measuredAt) })}</small>
    {/if}

    <div class="network-top">
      <div role="group" aria-labelledby="network-latency">
        {#if connected && data.internet.latency !== null}
          <strong class="num">{number.format(data.internet.latency)}<small>ms</small></strong>
        {:else}
          <strong class="absent">{connected ? t.network.noValue : "—"}</strong>
        {/if}
        <small id="network-latency">{t.network.internet}</small>
      </div>
      <button class="speed" disabled={!data.speedAvailable || data.speedRunning} onclick={runSpeed}
        aria-label={data.speedRunning ? t.network.testingSpeed : t.network.testSpeed} aria-describedby="network-speed-values">
        {#if speed}
          <b class="num">↓ {number.format(speed.download)} · ↑ {number.format(speed.upload)}</b>
        {:else}
          <b class="num" aria-hidden="true">↓ — · ↑ —</b>
        {/if}
        <span>Mbps · {t.network.test} <span aria-hidden="true">↻</span></span>
        <small>{speed ? interpolate(t.network.speedAt, { time: clock.format(speed.measuredAt) }) : t.network.noSpeedYet}</small>
      </button>
      <span id="network-speed-values" class="sr-only">{#if speed}{t.network.download}: {number.format(speed.download)} Mbps. {t.network.upload}: {number.format(speed.upload)} Mbps. {interpolate(t.network.speedAt, { time: date.format(speed.measuredAt) })}.{:else}{t.network.noSpeed}{/if}</span>
    </div>

    {#if connected}
      <p class="metrics">
        <span>{t.network.jitter} <b>{measured(data.internet.jitter, "ms")}</b></span>
        <span>{t.network.loss} <b>{measured(data.internet.loss, "%")}</b></span>
        <span>{t.network.gateway} <b>{measured(data.gateway.latency, "ms")}</b></span>
      </p>
      <small>{interpolate(t.network.gatewayBand, { jitter: measured(data.gateway.jitter, "ms"), loss: measured(data.gateway.loss, "%") })}</small>
    {/if}

    <section class="traffic-area" aria-labelledby="network-traffic">
      <h3 id="network-traffic" class="sr-only">{t.network.traffic}</h3>
      {#if history.length === 0}
        <small class="line">{t.network.noSamples}</small>
      {:else}
        <svg class="traffic" viewBox="0 0 {W} 72" preserveAspectRatio="none" role="img" aria-label="{t.network.chart}. {chartSummary}">
          {#if history.length === 1}
            <circle class="download" cx={x(0)} cy={y(history[0].download)} r="3" />
            <rect class="upload" x={x(0) - 3} y={y(history[0].upload) - 3} width="6" height="6" />
          {:else}
            <path class="download" d={path("download")} />
            <path class="upload" d={path("upload")} />
          {/if}
        </svg>
      {/if}
      <p class="traffic-values num">
        <span class="download"><span class="sr-only">{t.network.download}:&nbsp;</span><span aria-hidden="true">↓&nbsp;</span>{data.traffic ? rate(data.traffic.download) : t.network.noValue}</span>
        <span class="upload"><span class="sr-only">{t.network.upload}:&nbsp;</span><span aria-hidden="true">↑&nbsp;</span>{data.traffic ? rate(data.traffic.upload) : t.network.noValue}</span>
      </p>
    </section>

    {#if data.advancedUnavailable}
      <small class="line">{t.network.appsUnavailable}</small>
    {:else}
      <button class="detail-button" onclick={openApps} aria-haspopup="dialog">{appsLabel} <span aria-hidden="true">↗</span></button>
    {/if}
  {/if}
  <dialog bind:this={dialog} aria-labelledby="network-apps" oncancel={(event) => { event.preventDefault(); closeApps(); }}>
    <header><h2 id="network-apps">{t.network.appsDetail}</h2><button class="detail-button" onclick={closeApps}>{t.network.close} <span aria-hidden="true">×</span></button></header>
    {#if detailOpen}
      <section class="apps" aria-label={t.network.processTraffic}>
        {#if apps}
          {#if apps.count === 0}<p class="note">{t.network.noProcesses}</p>{/if}
          <ul aria-describedby="network-apps-scale">
            {#each apps.rows as app (app.others ? "others" : `app:${app.name}`)}
              <li class:others={app.others}>
                <b class="name" title={app.others ? t.network.othersHint : app.name}>{app.name}</b>
                <p class="num">↓ {rate(app.download)} · ↑ {rate(app.upload)}</p>
                <span class="bars" aria-hidden="true"><i style:width={width(app.download, apps.max)}></i><i class="up" style:width={width(app.upload, apps.max)}></i></span>
              </li>
            {/each}
          </ul>
          <small>{t.network.others}: {t.network.othersHint}</small>
          <small id="network-apps-scale">{t.network.appsScale}</small>
        {:else}
          <p class="note">{connected ? t.network.unmeasured : t.network.disconnected}</p>
        {/if}
      </section>
    {/if}
  </dialog>
</div>

<style>
  /* The free height of the card goes to the traffic chart. */
  .network { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .traffic-area { flex: 1; display: flex; flex-direction: column; }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  header { display: flex; align-items: center; gap: 10px; justify-content: space-between; min-height: 28px; margin-bottom: 6px; }
  h2 { font-size: 1.25em; font-weight: 500; text-transform: uppercase; letter-spacing: 1px; }
  small { font-size: .88em; color: var(--muted); }
  .line { display: block; margin-bottom: 4px; overflow-wrap: anywhere; }
  .line.alert, .line.warning { color: var(--warn); }
  .line.risk { color: var(--bad); }
  .line.accent { color: var(--accent); }
  .network-status { display: flex; flex-wrap: wrap; gap: 4px 10px; font-size: .88em; color: var(--muted); margin-bottom: 6px; }
  .network-top { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .network-top > div > small { display: block; }
  strong { display: block; font-size: 2.8em; font-weight: 400; line-height: 1.15; }
  strong small { font-size: .35em; margin-left: 3px; }
  strong.absent { font-size: 1.1em; line-height: 2.9; color: var(--muted); }
  .speed { text-align: right; display: grid; gap: 4px; }
  .speed span { font-size: .9em; color: var(--muted); }
  .speed:disabled { cursor: default; opacity: .6; }
  .speed:hover:not(:disabled) b { color: var(--accent); }
  .metrics { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 6px; margin: 6px 0 5px; font-size: .88em; }
  .metrics b { font-weight: 500; }
  .traffic { flex: 1; width: 100%; min-height: 40px; display: block; margin-top: 8px; background: linear-gradient(transparent 95%, var(--line) 95%); background-size: 100% 20px; }
  .traffic path { fill: none; stroke-width: 2; vector-effect: non-scaling-stroke; }
  .traffic .download { stroke: var(--accent); }
  .traffic .upload { stroke: var(--chart-secondary); stroke-dasharray: 5 3; }
  .traffic circle.download { fill: var(--accent); stroke: none; }
  .traffic rect.upload { fill: var(--chart-secondary); stroke: none; }
  .traffic-values { display: flex; justify-content: space-between; font-size: .9em; margin: 5px 0 2px; }
  .traffic-values .download { color: var(--accent); }
  .traffic-values .upload { color: var(--chart-secondary); }
  .detail-button { color: var(--accent); border-bottom: 1px solid var(--line); padding: 7px 0; text-align: left; margin-top: 5px; align-self: flex-start; }
  .speed:focus-visible, .detail-button:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  dialog { margin: auto; width: min(680px, 90vw); max-height: 80vh; overflow: auto; padding: 20px; color: var(--text); background: var(--surface); border: 1px solid var(--control-line); }
  dialog::backdrop { background: #000a; }
  dialog header { margin-bottom: 0; }
  dialog header .detail-button { margin-top: 0; }
  .apps { display: flex; flex-direction: column; gap: 8px; }
  .apps ul { margin: 0; padding: 0; list-style: none; }
  .apps li { margin-top: 12px; border-top: 1px solid var(--line); padding-top: 10px; overflow-wrap: anywhere; }
  .apps li p { margin-top: 8px; }
  .apps li.others .name { color: var(--muted); }
  .bars { display: grid; gap: 3px; margin-top: 6px; }
  .bars i { display: block; height: 4px; min-width: 1px; background: var(--accent); border-radius: 0 4px 4px 0; }
  .bars i.up { background: var(--chart-secondary); }
  .note { margin-top: 12px; color: var(--muted); }
</style>
