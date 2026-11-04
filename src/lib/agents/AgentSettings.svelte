<script lang="ts">
  import type { AgentsData, AgentNotificationPreferences } from "../core";
  import StatusLine, { type Tone } from "../settings/StatusLine.svelte";
  import Switch from "../settings/Switch.svelte";
  import { t } from "../strings";
  let { data, save, setHooks }: {
    data?: AgentsData;
    save: (preferences: AgentNotificationPreferences) => void;
    setHooks?: (enabled: boolean) => Promise<void>;
  } = $props();

  const uid = $props.id();
  let hooksEnabled = $state(false);
  let pending = $state(false);
  let error = $state(false);
  $effect(() => { hooksEnabled = data?.hooksEnabled ?? false; });

  const line = $derived.by((): { tone: Tone; text: string } => {
    if (!data) return { tone: "neutral", text: t.settings.welcome.checking };
    if (hooksEnabled) return { tone: "ok", text: t.agentHooks.on };
    if (data.codexWaitingUnavailable) return { tone: "warn", text: t.agents.codexWaitingUnavailable };
    return { tone: "neutral", text: t.agentHooks.off };
  });

  /** Hook files change only here, after the user clicks the switch. */
  async function changeHooks(enabled: boolean) {
    pending = true;
    error = false;
    try {
      if (!setHooks) throw new Error("unavailable");
      await setHooks(enabled);
      hooksEnabled = enabled;
    } catch { error = true; }
    finally { pending = false; }
  }

  let preferences = $state<AgentNotificationPreferences>({ waiting: true, done: true, failed: true });
  $effect(() => { preferences = data?.notificationPreferences ?? { waiting: true, done: true, failed: true }; });
  const switches = [
    { key: "waiting", label: t.agentNotifications.waitingSetting },
    { key: "done", label: t.agentNotifications.doneSetting },
    { key: "failed", label: t.agentNotifications.failedSetting },
  ] as const;
  function change(key: keyof AgentNotificationPreferences, enabled: boolean) {
    preferences = { ...preferences, [key]: enabled };
    save(preferences);
  }
</script>

<section aria-labelledby="{uid}-title">
  <h3 id="{uid}-title">{t.modules.agents}</h3>
  <StatusLine tone={line.tone} text={line.text} />
  <Switch label={t.agentHooks.setting} checked={hooksEnabled} disabled={pending}
    hint={hooksEnabled ? t.agentHooks.trust : undefined}
    error={error ? t.agentHooks.error : undefined}
    onchange={(enabled) => void changeHooks(enabled)} />
  <fieldset>
    <legend>{t.agentNotifications.groupTitle}</legend>
    {#if data?.notificationsAvailable === false}
      <p role="status">Notificações nativas indisponíveis nesta sessão.</p>
    {/if}
    {#each switches as item (item.key)}
      <Switch label={item.label} checked={preferences[item.key]} onchange={(enabled) => change(item.key, enabled)} />
    {/each}
  </fieldset>
</section>

<style>
  fieldset { margin-top: 18px; border: 0; }
  legend { margin-bottom: 8px; color: var(--muted); }
</style>
