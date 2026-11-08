<script lang="ts">
  import { onMount } from "svelte";
  import type { SystemData } from "../core";
  import { interpolate, t } from "../strings";
  import { pair } from "./format";
  import AbsentArea from "../AbsentArea.svelte";

  let { data, updatedAt }: { data: SystemData | null; updatedAt: number | null } = $props();

  /** CPU is sampled every 2 s while visible; a few missed samples make the reading old. */
  const STALE_AFTER = 10_000;

  let now = $state(Date.now());
  onMount(() => {
    const timer = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(timer);
  });

  const relative = new Intl.RelativeTimeFormat("pt-BR", { numeric: "auto" });
  function age(when: number) {
    const seconds = Math.max(0, Math.floor((now - when) / 1000));
    if (seconds < 60) return relative.format(-seconds, "second");
    if (seconds < 3600) return relative.format(-Math.floor(seconds / 60), "minute");
    return relative.format(-Math.floor(seconds / 3600), "hour");
  }
  /** When the reading is old, the moment it was taken; null while fresh. */
  const staleSince = $derived(updatedAt !== null && now - updatedAt > STALE_AFTER ? updatedAt : null);

  const celsius = new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  const percent = (used: number, total: number) => (total > 0 ? Math.round((used / total) * 100) : 0);

  /**
   * `note` is the GB pair B shows only under RAM; every meter keeps it in its accessible value.
   * `missing` keeps the slot of a meter the machine lacks, with the reason under an empty track.
   */
  type Meter = { name: string; used: number; total: number; warning: boolean; note: boolean; missing?: string };
  const unavailable = (metric: NonNullable<SystemData["unavailableMetrics"]>[number]) => data?.unavailableMetrics?.includes(metric) ?? false;

  /** The root disk represents the machine; the others only appear when they warn. */
  const disk = $derived(data?.disks.find((d) => d.mount === "/") ?? data?.disks[0] ?? null);
  const meters = $derived.by(() => {
    if (!data) return [];
    const list: Meter[] = [{ name: t.system.ram, ...data.ram, warning: false, note: true }];
    if (!unavailable("swap")) {
      list.push(data.swap ? { name: t.system.swap, ...data.swap, warning: false, note: false }
        : { name: t.system.swap, used: 0, total: 0, warning: false, note: false, missing: t.system.noSwap });
    }
    if (disk) list.push({ name: interpolate(t.system.disk, { mount: disk.mount }), used: disk.used, total: disk.total, warning: disk.warning, note: false });
    return list;
  });
  const fullDisks = $derived(data?.disks.filter((d) => d.warning) ?? []);
</script>

<div class="system">
  <header><h2 id="module-system-title">{t.modules.system}</h2></header>
  {#if !data}
    <AbsentArea absence={{ text: t.system.noReading, tone: "neutral" }} />
  {:else}
    {@const cpu = data.cpu}
    {@const battery = data.battery}
    {#if staleSince !== null}
      <small class="line alert" role="status"><span aria-hidden="true">⚠&nbsp;</span>{interpolate(t.system.stale, { age: age(staleSince) })}</small>
    {/if}
    {#if cpu.temperature?.warning}
      <small class="line alert"><span aria-hidden="true">⚠&nbsp;</span>{t.system.hot}</small>
    {/if}
    {#if fullDisks.length > 0}
      <div role="alert">
        {#each fullDisks as d (d.mount)}
          <small class="line alert"><span aria-hidden="true">⚠&nbsp;</span>{interpolate(t.system.diskFull, { mount: d.mount })}</small>
        {/each}
      </div>
    {/if}
    {#if battery?.warning}
      <small class="line alert"><span aria-hidden="true">⚠&nbsp;</span>{t.system.lowBattery}</small>
    {/if}

    <div class="readings" data-stale={staleSince !== null}>
    <div class="system-top">
      <div role="group" aria-labelledby="system-cpu">
        {#if cpu.usage === null}
          <strong class="absent">{t.system.measuring}</strong>
        {:else}
          <strong class="num">{Math.round(cpu.usage)}<small>%</small></strong>
        {/if}
        <small id="system-cpu">{t.system.cpu}</small>
      </div>
      <div class="temperature" role="group" aria-labelledby="system-temperature">
        {#if cpu.temperature}
          <strong class="num" class:warning={cpu.temperature.warning}>{celsius.format(cpu.temperature.celsius)}<small>°C</small></strong>
          <small id="system-temperature">{t.system.temperature}</small>
        {:else}
          <strong class="num empty" aria-hidden="true">—</strong>
          <small id="system-temperature">{unavailable("temperature") ? t.system.temperatureUnsupported : t.system.temperatureNoSensor}</small>
        {/if}
      </div>
    </div>

    {#if data.graphics}
      <div class="gpus" role="group" aria-label={t.system.gpus}>
        {#if data.graphics.failed}
          <small>{t.system.graphicsFailed}</small>
        {:else if data.graphics.devices.length === 0}
          <small>{t.system.noGpus}</small>
        {:else}
          {#each data.graphics.devices as gpu (gpu.label)}
            <section class="gpu" aria-label={gpu.label}>
              <h3>{gpu.label}</h3>
              <div class="gpu-values">
                <div role="group" aria-label={`${gpu.label} · ${t.system.gpuUsage}`}>
                  {#if gpu.usage.state === "ready"}<b class="num">{Math.round(gpu.usage.value)}%</b>
                  {:else}<small>{gpu.usage.state === "measuring" ? t.system.measuring : gpu.usage.state === "failed" ? t.system.gpuFailed : t.system.gpuUnavailable}</small>{/if}
                  <small>{t.system.gpuUsage}</small>
                </div>
                <div role="group" aria-label={`${gpu.label} · ${t.system.gpuTemperature}`}>
                  {#if gpu.temperature.state === "ready"}<b class="num">{celsius.format(gpu.temperature.value)}°C</b>
                  {:else}<small>{gpu.temperature.state === "failed" ? t.system.gpuFailed : t.system.gpuUnavailable}</small>{/if}
                  <small>{t.system.gpuTemperature}</small>
                </div>
              </div>
            </section>
          {/each}
        {/if}
      </div>
    {/if}

    {#each meters as m (m.name)}
      {@const value = percent(m.used, m.total)}
      {#if m.missing}
        <div class="meter">
          <div class="meter-label"><span>{m.name}</span><b class="num empty" aria-hidden="true">—</b></div>
          <div class="track" aria-hidden="true"></div>
          <small>{m.missing}</small>
        </div>
      {:else}
      <div class="meter">
        <div class="meter-label"><span>{m.name}</span><b class="num" class:warning={m.warning}>{value}%</b></div>
        <div class="track" class:warning={m.warning} role="meter" aria-label={m.name} aria-valuemin={0} aria-valuemax={100}
          aria-valuenow={value} aria-valuetext="{value}% · {pair(m.used, m.total)}">
          <i style:width="{value}%"></i>
        </div>
        {#if m.note}<small>{pair(m.used, m.total)}</small>{/if}
      </div>
      {/if}
    {/each}
    {#if unavailable("storage")}<small class="line">{t.system.storageUnavailable}</small>{/if}

    {#if battery}
      <p class="battery" class:warning={battery.warning}>
        <span aria-hidden="true">{"▰ "}</span>{t.system.battery} {Math.round(battery.percentage)}% · {t.system.statesBattery[battery.state]}
      </p>
    {:else}
      <p class="battery missing"><span aria-hidden="true">{"▱ "}</span>{unavailable("battery") ? t.system.batteryUnavailable : t.system.noBattery}</p>
    {/if}
    </div>
  {/if}
</div>

<style>
  /* The free height of the card spreads the meters. */
  .system { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  header { display: flex; align-items: center; gap: 10px; justify-content: space-between; min-height: 28px; margin-bottom: 6px; }
  h2 { font-size: 1.25em; font-weight: 500; text-transform: uppercase; letter-spacing: 1px; }
  small { font-size: .88em; color: var(--muted); }
  .line { display: block; margin-bottom: 4px; overflow-wrap: anywhere; }
  .line.alert { color: var(--warn); }
  .system-top { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .system-top > div > small { display: block; }
  .temperature { text-align: right; }
  .gpus { display: grid; gap: 6px; margin-top: 10px; }
  .gpu { border-top: 1px solid var(--line); padding-top: 5px; }
  .gpu h3 { font-size: .95em; font-weight: 500; margin-bottom: 4px; }
  .gpu-values { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  .gpu-values small { display: block; overflow-wrap: anywhere; }
  .gpu-values b { font-size: 1.3em; font-weight: 500; }
  strong { display: block; font-size: 2.8em; font-weight: 400; line-height: 1.15; }
  strong small { font-size: .35em; margin-left: 3px; }
  .temperature strong { font-size: 2.3em; }
  strong.absent { font-size: 1.1em; line-height: 2.9; color: var(--muted); }
  strong.warning, b.warning { color: var(--warn); }
  .meter { flex: 1; margin-top: 8px; display: flex; flex-direction: column; justify-content: center; }
  .meter-label { display: flex; justify-content: space-between; gap: 8px; margin-bottom: 4px; }
  .meter > small { display: block; margin-top: 4px; }
  .track { height: 5px; background: var(--line); border-radius: 5px; }
  .track i { display: block; height: 100%; background: var(--accent); border-radius: inherit; }
  .track.warning i { background: var(--warn); }
  .battery { margin-top: 4px; color: var(--ok); }
  .battery.warning { color: var(--warn); }
  .battery.missing, .empty { color: var(--muted); }
  /* The free height of the card spreads the meters, also under the conditional lines. */
  .readings { flex: 1; display: flex; flex-direction: column; }
  .readings[data-stale="true"] { --text: var(--muted); --accent: var(--muted); --ok: var(--muted); color: var(--muted); }
</style>
