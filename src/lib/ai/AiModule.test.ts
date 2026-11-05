import { render, screen, fireEvent, within } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import AiModule from "./AiModule.svelte";
import type { AiData } from "../core";
import asymmetric from "./fixtures/asymmetric.json";
import usage from "./fixtures/claude-usage.json";

/** Instant the oauth/usage fixtures were published (São Paulo). */
const USAGE_NOW = 1793401200000;

beforeEach(() => {
  vi.stubEnv("TZ", "America/Sao_Paulo");
  vi.useFakeTimers();
  vi.setSystemTime(USAGE_NOW);
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllEnvs();
});

const provider = (name: string) => screen.getByRole("region", { name });

it("shows the remaining five hour allowance and reset for each provider", () => {
  render(AiModule, { data: usage as AiData, refresh: vi.fn() });
  const codex = within(provider("Codex")).getByRole("meter", { name: "Codex · janela de 5 h" });
  expect(codex).toHaveAttribute("aria-valuenow", "9");
  expect(codex).toHaveTextContent("9%");
  expect(within(provider("Codex")).getByText("Restante em 5 h · reinicia 14:22")).toBeInTheDocument();
  const claude = within(provider("Claude")).getByRole("meter", { name: "Claude · janela de 5 h" });
  expect(claude).toHaveAttribute("aria-valuenow", "85");
  expect(within(provider("Claude")).getByText("Restante em 5 h · reinicia 23:30")).toBeInTheDocument();
});

it("shows a weekly bar per provider from the all models bucket only", () => {
  render(AiModule, { data: usage as AiData, refresh: vi.fn() });
  const codex = within(provider("Codex")).getByRole("meter", { name: "Codex · semana" });
  expect(codex).toHaveAttribute("aria-valuenow", "52");
  expect(within(provider("Codex")).getByText("52%")).toBeInTheDocument();
  expect(within(provider("Codex")).getByText("Restante na semana")).toBeInTheDocument();
  // Claude's per model weekly limits (Sonnet 67% remaining) stay out of the panel.
  expect(within(provider("Claude")).getAllByRole("meter", { name: "Claude · semana" })
    .map((meter) => meter.getAttribute("aria-valuenow"))).toEqual(["94"]);
  expect(screen.queryByText("Sonnet")).not.toBeInTheDocument();
  expect(screen.queryByText("67%")).not.toBeInTheDocument();
  expect(within(provider("Codex")).getByText("Reinício 04/11/2026")).toBeInTheDocument();
  expect(within(provider("Claude")).getByText("Reinício 05/11/2026")).toBeInTheDocument();
});

it("shows only the reading time as in prototype B, without source, age or plans", () => {
  vi.setSystemTime(USAGE_NOW + 420_000);
  render(AiModule, { data: usage as AiData, refresh: vi.fn() });
  expect(within(provider("Claude")).getByText("Leitura 20:00")).toBeInTheDocument();
  expect(within(provider("Codex")).getByText("Leitura 12:34")).toBeInTheDocument();
  expect(screen.queryByText(/atualizado|oauth\/usage|app-server/)).not.toBeInTheDocument();
  expect(screen.queryByText("Pro")).not.toBeInTheDocument();
  expect(screen.queryByText("Plus")).not.toBeInTheDocument();
});

it("removes a restarted window's arc and never shows it as 0%", async () => {
  const { default: reset } = await import("./fixtures/reset.json");
  vi.setSystemTime(1793467355000 + 60_000);
  render(AiModule, { data: reset as AiData, refresh: vi.fn() });
  const codex = provider("Codex");
  expect(within(codex).getByText("Reiniciado às 14:22 · sem leitura nova")).toBeInTheDocument();
  expect(within(codex).queryByRole("meter", { name: "Codex · janela de 5 h" })).not.toBeInTheDocument();
  expect(within(codex).getByRole("img", { name: "Codex · janela de 5 h: Reiniciado às 14:22 · sem leitura nova" })).toBeInTheDocument();
  expect(within(codex).queryByText(/^0/)).not.toBeInTheDocument();
  expect(within(codex).getByRole("meter", { name: "Codex · semana" })).toHaveAttribute("aria-valuenow", "52");
});

it("removes a restarted weekly bar and keeps the five hour reading", async () => {
  const { default: weeklyReset } = await import("./fixtures/weekly-reset.json");
  render(AiModule, { data: weeklyReset as AiData, refresh: vi.fn() });
  const codex = provider("Codex");
  expect(within(codex).queryByRole("meter", { name: "Codex · semana" })).not.toBeInTheDocument();
  expect(within(codex).getByText("Reiniciado às 14:00 · sem leitura nova")).toBeInTheDocument();
  expect(within(codex).getByRole("meter", { name: "Codex · janela de 5 h" })).toHaveAttribute("aria-valuenow", "9");
});

it("keeps an old reading with its time, naming the date once it is a day old", async () => {
  const { default: stale } = await import("./fixtures/stale.json");
  vi.setSystemTime(1793460871000);
  const view = render(AiModule, { data: stale as AiData, refresh: vi.fn() });
  expect(within(provider("Codex")).getByText("Leitura 11:33")).toBeInTheDocument();
  expect(within(provider("Codex")).getByRole("meter", { name: "Codex · janela de 5 h" })).toHaveAttribute("aria-valuenow", "9");
  view.unmount();
  const old = structuredClone(stale) as AiData;
  old.codex.reading!.readAt -= 86_400_000;
  render(AiModule, { data: old, refresh: vi.fn() });
  expect(within(provider("Codex")).getByText("Leitura 30/10, 11:33")).toBeInTheDocument();
});

it("omits an uninstalled CLI and gives the installed provider the available space", async () => {
  const { default: missing } = await import("./fixtures/missing.json");
  const data = { ...(missing as AiData), claude: usage.claude as AiData["claude"] };
  render(AiModule, { data, refresh: vi.fn() });
  expect(screen.queryByRole("region", { name: "Codex" })).not.toBeInTheDocument();
  expect(within(provider("Claude")).getByRole("meter", { name: "Claude · janela de 5 h" })).toBeInTheDocument();
});

it("explains when Codex has no reading yet", async () => {
  const { default: empty } = await import("./fixtures/empty.json");
  render(AiModule, { data: empty as AiData, refresh: vi.fn() });
  expect(within(provider("Codex")).getByText("Sem leitura dos limites do Codex ainda · use o Codex ou atualize com ↻")).toBeInTheDocument();
  expect(within(provider("Codex")).queryByRole("meter")).not.toBeInTheDocument();
});

it("shows sustained Codex failures and keeps the last reading with its age", async () => {
  const { default: failure } = await import("./fixtures/failure.json");
  vi.setSystemTime(1793460871000 + 90_000);
  render(AiModule, { data: failure as AiData, refresh: vi.fn() });
  const codex = provider("Codex");
  expect(within(codex).getByText(/Falha ao atualizar/)).toHaveTextContent("⚠ Falha ao atualizar há 1 minuto");
  expect(within(codex).getByText("Leitura 11:33")).toBeInTheDocument();
  expect(within(codex).getByRole("meter", { name: "Codex · janela de 5 h" })).toBeInTheDocument();
});

it("hides Codex failures while paused offline", async () => {
  const { default: failure } = await import("./fixtures/failure.json");
  render(AiModule, { data: { ...(failure as AiData), paused: true }, refresh: vi.fn() });
  expect(screen.queryByText(/Falha ao atualizar/)).not.toBeInTheDocument();
});

/** Instant the status line fixtures were published, before both windows reset. */
const STATUS_LINE_NOW = 1738420000000;

it("shows Claude's windows from the status line", async () => {
  const { default: connected } = await import("./fixtures/claude-connected.json");
  vi.setSystemTime(STATUS_LINE_NOW);
  render(AiModule, { data: connected as AiData, refresh: vi.fn() });
  const claude = provider("Claude");
  expect(within(claude).getByText("Leitura 11:26")).toBeInTheDocument();
  expect(within(claude).getByRole("meter", { name: "Claude · janela de 5 h" })).toHaveAttribute("aria-valuenow", "77");
  expect(within(claude).getByRole("meter", { name: "Claude · semana" })).toHaveAttribute("aria-valuenow", "59");
  expect(within(claude).queryByRole("status")).not.toBeInTheDocument();
});

it("warns when another tool replaced the status line and reconnects only on click", async () => {
  const { default: replaced } = await import("./fixtures/claude-replaced.json");
  vi.setSystemTime(STATUS_LINE_NOW + 3_600_000);
  const reconnect = vi.fn();
  render(AiModule, { data: replaced as AiData, refresh: vi.fn(), reconnect });
  const claude = provider("Claude");
  expect(within(claude).getByRole("status")).toHaveTextContent("⚠ Status line desconectada · Reconectar");
  expect(within(claude).getByText("Leitura 11:26")).toBeInTheDocument();
  expect(reconnect).not.toHaveBeenCalled();
  await fireEvent.click(within(claude).getByRole("button", { name: "Reconectar" }));
  expect(reconnect).toHaveBeenCalledOnce();
});

it("asks for a Claude Code session when connected without a reading", async () => {
  const { default: claudeEmpty } = await import("./fixtures/claude-empty.json");
  render(AiModule, { data: claudeEmpty as AiData, refresh: vi.fn() });
  const claude = provider("Claude");
  expect(within(claude).getByText("Abra uma sessão do Claude Code para ler os limites")).toBeInTheDocument();
  expect(within(claude).queryByRole("meter")).not.toBeInTheDocument();
  expect(within(provider("Codex")).getByRole("meter", { name: "Codex · janela de 5 h" })).toBeInTheDocument();
});

it("points to Settings before connecting to Claude Code", async () => {
  const { default: off } = await import("./fixtures/claude-off.json");
  const configure = vi.fn();
  render(AiModule, { data: off as AiData, refresh: vi.fn(), configure });
  const claude = provider("Claude");
  expect(within(claude).getByText(/Conecte o app ao Claude Code para ler os limites/)).toBeInTheDocument();
  expect(within(claude).queryByRole("meter")).not.toBeInTheDocument();
  await fireEvent.click(within(claude).getByRole("button", { name: "Configurar" }));
  expect(configure).toHaveBeenCalledOnce();
});

it("omits Claude Code when it is not installed", async () => {
  const { default: claudeMissing } = await import("./fixtures/claude-missing.json");
  render(AiModule, { data: claudeMissing as AiData, refresh: vi.fn() });
  expect(screen.queryByRole("region", { name: "Claude" })).not.toBeInTheDocument();
  expect(provider("Codex")).toBeInTheDocument();
});

it("shows an expired Claude token and keeps the status line reading", async () => {
  const { default: expired } = await import("./fixtures/claude-expired.json");
  vi.setSystemTime(STATUS_LINE_NOW);
  render(AiModule, { data: expired as AiData, refresh: vi.fn() });
  const claude = provider("Claude");
  expect(within(claude).getByRole("status")).toHaveTextContent("⚠ Token do Claude expirado · abra o Claude Code");
  expect(within(claude).getByText("Leitura 20:00")).toBeInTheDocument();
  expect(within(claude).getByRole("meter", { name: "Claude · janela de 5 h" })).toHaveAttribute("aria-valuenow", "80");
});

it("shows sustained oauth/usage failures and keeps the last reading", async () => {
  const { default: failure } = await import("./fixtures/claude-failure.json");
  vi.setSystemTime(USAGE_NOW + 420_000);
  render(AiModule, { data: failure as AiData, refresh: vi.fn() });
  const claude = provider("Claude");
  expect(within(claude).getByRole("status")).toHaveTextContent("⚠ Falha ao atualizar há 2 minutos");
  expect(within(claude).getByText("Leitura 20:00")).toBeInTheDocument();
  expect(within(provider("Codex")).queryByText(/Falha/)).not.toBeInTheDocument();
});

it("hides oauth/usage failures while Claude is paused offline", async () => {
  const { default: failure } = await import("./fixtures/claude-failure.json");
  const paused = { ...(failure as AiData), claude: { ...(failure as AiData).claude!, paused: true } };
  render(AiModule, { data: paused, refresh: vi.fn() });
  expect(screen.queryByText(/Falha ao atualizar/)).not.toBeInTheDocument();
});

it("refreshes on demand and waits ten seconds between clicks", async () => {
  const refresh = vi.fn();
  render(AiModule, { data: usage as AiData, refresh });
  const button = screen.getByRole("button", { name: "Atualizar IA" });
  await fireEvent.click(button);
  expect(refresh).toHaveBeenCalledOnce();
  expect(button).toBeDisabled();
  await vi.advanceTimersByTimeAsync(9_000);
  expect(button).toBeDisabled();
  await vi.advanceTimersByTimeAsync(1_000);
  expect(button).toBeEnabled();
});

it("disables refresh while refreshing or offline and keeps the reading time", () => {
  const { unmount } = render(AiModule, { data: { ...(usage as AiData), refreshing: true }, refresh: vi.fn() });
  expect(screen.getByRole("button", { name: "Atualizar IA" })).toBeDisabled();
  expect(screen.getAllByRole("status")[0]).toHaveTextContent("Atualizando…");
  unmount();
  render(AiModule, { data: { ...(usage as AiData), paused: true }, refresh: vi.fn() });
  expect(screen.getByRole("button", { name: "Atualizar IA" })).toBeDisabled();
  expect(within(provider("Claude")).getByText("Leitura 20:00")).toBeInTheDocument();
});

it("opens Settings from the module", async () => {
  const configure = vi.fn();
  render(AiModule, { data: usage as AiData, refresh: vi.fn(), configure });
  await fireEvent.click(screen.getByRole("button", { name: "Configurar IA" }));
  expect(configure).toHaveBeenCalledOnce();
});

it("names high usage only to assistive technology, as prototype B shows no line for it", () => {
  render(AiModule, { data: usage as AiData, refresh: vi.fn() });
  const codex = within(provider("Codex")).getByRole("meter", { name: "Codex · janela de 5 h" });
  expect(codex).toHaveAttribute("aria-valuetext", "9% restantes · uso alto");
  expect(codex).not.toHaveTextContent("uso alto");
  expect(within(provider("Codex")).queryByRole("status")).not.toBeInTheDocument();
  expect(screen.queryByText(/uso alto/i)).not.toBeInTheDocument();
  expect(within(provider("Codex")).getByRole("meter", { name: "Codex · semana" })).toHaveAttribute("aria-valuetext", "52% restantes");
  expect(within(provider("Claude")).queryByRole("status")).not.toBeInTheDocument();
  expect(provider("Codex").style.getPropertyValue("--instrument")).toBe("var(--accent)");
  expect(provider("Claude").style.getPropertyValue("--instrument")).toBe("var(--band-lavender-text)");
});

it("titles the shared card in prototype B's words, without source or update subtitle", () => {
  render(AiModule, { data: usage as AiData, refresh: vi.fn() });
  expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("IA e Agentes");
  expect(screen.queryByText("Codex e Claude Code")).not.toBeInTheDocument();
  expect(screen.queryByText("consumido")).not.toBeInTheDocument();
  expect(screen.queryByText("Janela de 5 h")).not.toBeInTheDocument();
});

it("shows remaining weekly allowance when the reading has no five hour window", () => {
  const onlyWeekly = structuredClone(usage) as AiData;
  // The app-server may put a lone weekly window in the primary slot.
  const bucket = onlyWeekly.codex.reading!.buckets[0];
  bucket.primary = bucket.secondary;
  bucket.secondary = null;
  render(AiModule, { data: onlyWeekly, refresh: vi.fn() });
  const codex = provider("Codex");
  expect(within(codex).queryByRole("img", { name: /janela de 5 h/ })).not.toBeInTheDocument();
  const weekly = within(codex).getByRole("meter", { name: "Codex · semana" });
  expect(weekly).toHaveTextContent("52%");
  expect(weekly).toHaveAttribute("aria-valuenow", "52");
  expect(weekly).toHaveAttribute("aria-valuetext", "52% restantes");
});

it("never labels a window of another duration as five hours or weekly", () => {
  const other = structuredClone(usage) as AiData;
  other.codex.reading!.buckets[0].primary = { usedPercent: 40, durationMins: 60, resetsAt: null, reset: false };
  render(AiModule, { data: other, refresh: vi.fn() });
  expect(within(provider("Codex")).queryByRole("meter", { name: "Codex · janela de 5 h" })).not.toBeInTheDocument();
  expect(within(provider("Codex")).queryByText("60%")).not.toBeInTheDocument();
});

it("describes compact title actions on focus and pointer without changing focus on update", async () => {
  const { rerender } = render(AiModule, { data: usage as AiData, refresh: vi.fn() });
  const refresh = screen.getByRole("button", { name: "Atualizar IA" });
  expect(refresh).toHaveAccessibleDescription("Consultar os limites de uso de Claude e Codex");
  expect(screen.getByRole("button", { name: "Configurar IA" })).toHaveAccessibleDescription("Abrir IA e Agentes nas Configurações");
  refresh.focus();
  expect(refresh).toHaveFocus();
  await rerender({ data: { ...(usage as AiData), refreshing: true } });
  expect(refresh).toHaveFocus();
});

it("identifies offline providers while preserving cached instruments and reading time", async () => {
  const { default: offline } = await import("./fixtures/offline.json");
  render(AiModule, { data: offline as AiData, refresh: vi.fn() });
  for (const name of ["Codex", "Claude"]) {
    const region = within(provider(name));
    expect(region.getByText("Coleta pausada · offline ou aguardando retomada")).toHaveAttribute("role", "status");
    expect(region.getByRole("meter", { name: `${name} · janela de 5 h` })).toBeInTheDocument();
    expect(region.getByText(/^Leitura \d\d:\d\d$/)).toBeInTheDocument();
  }
});

it("distinguishes loading without a reading from a completed empty snapshot", async () => {
  const { default: loading } = await import("./fixtures/loading.json");
  const { rerender } = render(AiModule, { data: loading as AiData, refresh: vi.fn() });
  expect(screen.getByText("Atualizando…")).toHaveAttribute("role", "status");
  expect(screen.queryByRole("meter")).not.toBeInTheDocument();
  await rerender({ data: { ...(loading as AiData), refreshing: false } });
  expect(screen.queryByText("Atualizando…")).not.toBeInTheDocument();
  expect(within(provider("Codex")).getByText(/Sem leitura/)).toBeInTheDocument();
});

it("explains a missing weekly value without claiming a restart", async () => {
  const { default: unavailable } = await import("./fixtures/weekly-unavailable.json");
  render(AiModule, { data: unavailable as AiData, refresh: vi.fn() });
  expect(within(provider("Codex")).queryByRole("meter", { name: "Codex · semana" })).not.toBeInTheDocument();
  expect(within(provider("Codex")).getByText("Janela semanal indisponível")).toBeInTheDocument();
  expect(within(provider("Codex")).queryByText(/Reiniciado/)).not.toBeInTheDocument();
});


it("removes only the elapsed window while awaiting a new snapshot", async () => {
  vi.setSystemTime(1793413799000);
  render(AiModule, { data: usage as AiData, refresh: vi.fn() });
  expect(within(provider("Claude")).getByRole("meter", { name: "Claude · janela de 5 h" })).toHaveAttribute("aria-valuenow", "85");
  await vi.advanceTimersByTimeAsync(1000);
  expect(within(provider("Claude")).queryByRole("meter", { name: "Claude · janela de 5 h" })).not.toBeInTheDocument();
  expect(within(provider("Claude")).getByText("Reiniciado às 23:30 · sem leitura nova")).toBeInTheDocument();
  expect(within(provider("Claude")).getByRole("meter", { name: "Claude · semana" })).toHaveAttribute("aria-valuenow", "94");
  expect(within(provider("Codex")).getByRole("meter", { name: "Codex · janela de 5 h" })).toHaveAttribute("aria-valuenow", "9");
});


it("dismisses action descriptions with Escape before the panel handles it", async () => {
  render(AiModule, { data: usage as AiData, refresh: vi.fn() });
  const configure = screen.getByRole("button", { name: "Configurar IA" });
  configure.focus();
  const escape = new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true });
  configure.dispatchEvent(escape);
  await fireEvent.keyUp(configure, { key: "Escape" });
  expect(escape.defaultPrevented).toBe(true);
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  expect(configure).toHaveFocus();
});

describe("automatic composition without a reading", () => {
  const area = (name: string) => provider(name).querySelector<HTMLElement>(".area");

  it("shows a neutral area for the installed provider awaiting its first reading", () => {
    render(AiModule, { data: { refreshing: false, codex: { state: "empty", plan: null, reading: null } }, refresh: vi.fn() });
    expect(provider("Codex")).toHaveTextContent(/^Codex/);
    expect(area("Codex")).toHaveAttribute("data-tone", "neutral");
    expect(area("Codex")).toHaveTextContent("◌ Sem leitura dos limites do Codex ainda · use o Codex ou atualize com ↻");
    expect(screen.queryByRole("region", { name: "Claude" })).not.toBeInTheDocument();
    expect(screen.queryByRole("meter")).not.toBeInTheDocument();
  });

  it("keeps an installed provider visible when its account is unavailable", async () => {
    const { default: missing } = await import("./fixtures/missing.json");
    const data: AiData = { ...missing as AiData, codex: { ...missing.codex as AiData["codex"], installed: true } };
    render(AiModule, { data, refresh: vi.fn() });
    expect(area("Codex")).toHaveAttribute("data-tone", "warn");
    expect(area("Codex")).toHaveTextContent("Codex instalado · verifique a sessão no CLI");
  });

  it("offers a new attempt when the first Codex reading failed", async () => {
    const refresh = vi.fn();
    const data: AiData = { refreshing: false, codex: { state: "empty", plan: null, reading: null, failureSince: USAGE_NOW - 600_000 } };
    render(AiModule, { data, refresh });
    expect(area("Codex")).toHaveAttribute("data-tone", "warn");
    expect(area("Codex")).toHaveTextContent("Falha ao atualizar há 10 minutos · sem leitura anterior");
    await fireEvent.click(within(area("Codex")!).getByRole("button", { name: "Tentar de novo" }));
    expect(refresh).toHaveBeenCalledOnce();
  });

  it("names Claude's reason without a reading: expired, replaced, failed or not connected", async () => {
    const reconnect = vi.fn();
    const claude = (patch: Partial<NonNullable<AiData["claude"]>>): AiData => ({
      ...(usage as AiData), claude: { ...(usage as AiData).claude!, reading: null, ...patch },
    });
    const { unmount } = render(AiModule, { data: claude({ usage: "expired" }), refresh: vi.fn() });
    expect(area("Claude")).toHaveTextContent("⚠ Token do Claude expirado · abra o Claude Code");
    unmount();
    const replaced = render(AiModule, { data: claude({ usage: "off", statusLine: "replaced" }), refresh: vi.fn(), reconnect });
    await fireEvent.click(within(area("Claude")!).getByRole("button", { name: "Reconectar" }));
    expect(reconnect).toHaveBeenCalledOnce();
    replaced.unmount();
    const failed = render(AiModule, { data: claude({ usage: "on", failureSince: USAGE_NOW - 120_000 }), refresh: vi.fn() });
    expect(area("Claude")).toHaveTextContent("Falha ao atualizar há 2 minutos · sem leitura anterior Tentar de novo");
    failed.unmount();
    render(AiModule, { data: claude({ usage: "off", statusLine: "off" }), refresh: vi.fn() });
    expect(area("Claude")).toHaveAttribute("data-tone", "neutral");
    expect(area("Claude")).toHaveTextContent("Conecte o app ao Claude Code para ler os limites Configurar");
  });

  it("dims an old reading under its failure line instead of the area", async () => {
    const { default: failure } = await import("./fixtures/failure.json");
    render(AiModule, { data: failure as AiData, refresh: vi.fn() });
    expect(area("Codex")).toBeNull();
    expect(provider("Codex")).toHaveAttribute("data-stale", "true");
    expect(provider("Claude")).not.toHaveAttribute("data-stale", "true");
  });
});

it.each([true, false])("keeps missing-token guidance and an action after the strip expires (reading: %s)", async (hasReading) => {
  const { default: OperationsStrip } = await import("../OperationsStrip.svelte");
  const { act } = await import("@testing-library/svelte");
  const data: AiData = structuredClone(usage) as AiData;
  data.claude!.usageUnavailable = true;
  if (!hasReading) data.claude!.reading = null;
  const configure = vi.fn();
  render(AiModule, { data, refresh: vi.fn(), configure });
  const strip = render(OperationsStrip, { snapshots: {}, ready: true });
  await strip.rerender({ snapshots: { ai: { module: "ai", updatedAt: null, data: { ...data, claude: { ...data.claude, operation: { id: 2, origin: "automatic", state: "unavailable" } } } } }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Claude · Atualização indisponível");
  await act(() => vi.advanceTimersByTime(4000));
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  expect(provider("Claude")).toHaveTextContent("Token do Claude indisponível · abra o Claude Code");
  await fireEvent.click(within(provider("Claude")).getByRole("button", { name: "Configurar" }));
  expect(configure).toHaveBeenCalledOnce();
  expect(within(provider("Codex")).getByRole("meter", { name: "Codex · janela de 5 h" })).toHaveAttribute("aria-valuenow", "9");
});

// Snapshot fixtures are also exercised in the browser for geometry.
it.each(asymmetric)("preserves unknown usage and the other provider in $name", ({ provider: key, slot, state, data }) => {
  render(AiModule, { data: data as AiData, refresh: vi.fn() });
  const name = key === "codex" ? "Codex" : "Claude";
  const other = key === "codex" ? "Claude" : "Codex";
  const label = slot === "primary" ? "janela de 5 h" : "semana";
  expect(within(provider(name)).queryByRole("meter", { name: `${name} · ${label}` })).not.toBeInTheDocument();
  expect(within(provider(name)).queryByText("0%")).not.toBeInTheDocument();
  expect(within(provider(other)).getAllByRole("meter")).toHaveLength(2);
  const region = within(provider(name));
  if (state === "reset") {
    expect(region.getByText(/Reiniciado às.*sem leitura nova/)).toBeInTheDocument();
  } else {
    expect(region.queryByRole("img", { name: new RegExp(label) })).not.toBeInTheDocument();
    expect(region.getAllByRole("meter")).toHaveLength(1);
  }
});
