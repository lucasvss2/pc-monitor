<script lang="ts">
  import type { AiData } from "../core";
  import FieldError from "../settings/FieldError.svelte";
  import StatusLine, { type Tone } from "../settings/StatusLine.svelte";
  import { interpolate, t } from "../strings";
  import { installed, usageWindows } from "./profile";
  let { data, connect, disconnect, setUsage }: {
    data?: AiData; connect: () => Promise<void>; disconnect: () => Promise<void>;
    setUsage: (enabled: boolean) => Promise<void>;
  } = $props();

  /** Claude's two controls: the status line and the oauth/usage opt-in; each failure shows right below its own. */
  type Control = "statusLine" | "usage";

  let busy = $state(false);
  /** The warning about the undocumented endpoint is open. */
  let warning = $state(false);
  let errors = $state<Partial<Record<Control, string>>>({});
  const uid = $props.id();
  const errorId = (control: Control) => `${uid}-${control}-error`;
  const describedBy = (control: Control) => (errors[control] ? errorId(control) : undefined);
  const claude = $derived(data?.claude);
  function windowsLabel(provider: AiData["codex"] | AiData["claude"]) {
    if (!provider || !installed(provider)) return null;
    const { five, week } = usageWindows(provider.reading);
    const windows = [five && t.ai.fiveHour, week && t.ai.settings.weeklyWindow].filter(Boolean);
    return windows.length > 0 ? interpolate(t.ai.settings.automaticWindows, { windows: windows.join(" · ") }) : null;
  }
  const claudeWindows = $derived(windowsLabel(claude));
  const codexWindows = $derived(windowsLabel(data?.codex));

  type State = { tone: Tone; text: string };
  const codexState = $derived.by((): State => {
    if (!data || data.codex.installed === null) return { tone: "neutral", text: t.settings.welcome.checking };
    if (data.codex.state === "missing") return { tone: "warn", text: installed(data.codex) ? t.ai.codexNeedsSession : t.ai.missingCodex };
    if (data.codex.state === "empty") return { tone: "warn", text: t.ai.emptyCodex };
    return { tone: "ok", text: data.codex.plan ? interpolate(t.settings.codexPlan, { plan: data.codex.plan }) : t.settings.codexDetected };
  });
  const statusLineText = $derived(claude?.statusLine === "connected" ? t.ai.settings.connected
    : claude?.statusLine === "replaced" ? t.ai.settings.replaced : t.ai.settings.off);
  const claudeState = $derived.by((): State => {
    if (!data || claude?.installed === null) return { tone: "neutral", text: t.settings.welcome.checking };
    if (claude?.state === "missing" && !installed(claude)) return { tone: "warn", text: t.ai.missingClaude };
    return { tone: claude?.statusLine === "connected" ? "ok" : "warn", text: statusLineText };
  });

  async function run(control: Control, action: () => Promise<void>) {
    if (busy) return;
    busy = true;
    errors[control] = undefined;
    try {
      await action();
    } catch (code) {
      errors[control] = t.ai.settings.errors[code as keyof typeof t.ai.settings.errors] ?? t.ai.settings.errors.unwritable;
    } finally { busy = false; }
  }
</script>

<section aria-labelledby="{uid}-title">
  <h3 id="{uid}-title">{t.modules.ai}</h3>

  <section aria-labelledby="{uid}-claude">
    <h4 id="{uid}-claude">{t.ai.claude}</h4>
    <StatusLine tone={claudeState.tone} text={claudeState.text} />
    {#if claudeWindows}<p class="hint">{claudeWindows}</p>{/if}
    {#if claude?.state === "missing" && claude.statusLine !== "off"}<p class="hint">{statusLineText}</p>{/if}
    {#if claude?.statusLine === "connected"}
      <button disabled={busy} aria-describedby={describedBy("statusLine")} onclick={() => run("statusLine", disconnect)}>{t.ai.settings.disconnect}</button>
    {:else if claude?.statusLine === "replaced"}
      <button disabled={busy} aria-describedby={describedBy("statusLine")} onclick={() => run("statusLine", connect)}>{t.ai.reconnect}</button>
      <button disabled={busy} aria-describedby={describedBy("statusLine")} onclick={() => run("statusLine", disconnect)}>{t.ai.settings.disconnect}</button>
    {:else}
      <p class="hint">{t.ai.settings.explanation}</p>
      <button disabled={busy} aria-describedby={describedBy("statusLine")} onclick={() => run("statusLine", connect)}>{t.ai.settings.connect}</button>
    {/if}
    {#if errors.statusLine}<FieldError id={errorId("statusLine")} message={errors.statusLine} />{/if}

    <h5>{t.ai.settings.usage.title}</h5>
    {#if claude?.usage === "unsupported"}
      <p class="hint">{t.ai.settings.usage.unsupported}</p>
    {:else if claude?.usage === "on" || claude?.usage === "expired"}
      <p>{claude.usage === "expired" ? t.ai.expired : t.ai.settings.usage.on}</p>
      <button disabled={busy} aria-describedby={describedBy("usage")} onclick={() => run("usage", () => setUsage(false))}>{t.ai.settings.usage.disable}</button>
    {:else if warning}
      <div role="alertdialog" aria-labelledby="{uid}-warning" class="warning">
        <b id="{uid}-warning">{t.ai.settings.usage.warningTitle}</b>
        <p>{t.ai.settings.usage.warning}</p>
        <button disabled={busy} aria-describedby={describedBy("usage")} onclick={() => run("usage", async () => { await setUsage(true); warning = false; })}>{t.ai.settings.usage.confirm}</button>
        <button disabled={busy} onclick={() => (warning = false)}>{t.ai.settings.usage.cancel}</button>
      </div>
    {:else}
      <p class="hint">{t.ai.settings.usage.explanation}</p>
      <button disabled={busy} onclick={() => (warning = true)}>{t.ai.settings.usage.enable}</button>
    {/if}
    {#if errors.usage}<FieldError id={errorId("usage")} message={errors.usage} />{/if}
  </section>

  <section aria-labelledby="{uid}-codex">
    <h4 id="{uid}-codex">{t.ai.codex}</h4>
    <StatusLine tone={codexState.tone} text={codexState.text} />
    {#if codexWindows}<p class="hint">{codexWindows}</p>{/if}
  </section>
</section>

<style>
  .hint { color: var(--muted); }
</style>
