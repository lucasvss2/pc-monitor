import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import AgentsModule from "./AgentsModule.svelte";
import type { AgentsData } from "../core";
import empty from "./fixtures/empty.json";
import waiting from "./fixtures/waiting.json";
import mixed from "./fixtures/mixed.json";
import states from "./fixtures/states.json";

const list = () => screen.getByRole("list", { name: "Sessões de agente" });
const row = (name: string) => within(list()).getAllByRole("listitem").find((item) => within(item).queryByText(name))!;
const legend = (label: string) => screen.getByRole("button", { name: new RegExp(`^${label} (\\d+|—)$`) });
const counter = (label: string) => legend(label).querySelector("b")?.textContent;
const ring = () => screen.getByRole("button", { name: "Sessões de agente" });

it("shows an explicit empty ring when no sessions are active and no waiting alert", () => {
  render(AgentsModule, { data: empty as AgentsData });
  expect(screen.getByText("Nenhuma sessão de agente ativa")).toBeInTheDocument();
  expect(ring()).toHaveTextContent("0");
  expect(ring().querySelectorAll("circle[stroke-dasharray]")).toHaveLength(0);
  expect(screen.queryByText(/⚠ \d+ aguardando você/)).not.toBeInTheDocument();
  expect(screen.queryByRole("list", { name: "Sessões de agente" })).not.toBeInTheDocument();
});

it("keeps the notice space with a neutral line when no session waits for you", async () => {
  render(AgentsModule, { data: { ...states, sessions: states.sessions.filter((s) => s.state !== "waiting") } as AgentsData });
  const calm = screen.getByRole("button", { name: /Nenhuma sessão aguardando você/ });
  expect(calm).toHaveTextContent("✓ Nenhuma sessão aguardando você");
  expect(calm).toHaveTextContent("Ver sessões ↗");
  await userEvent.click(calm);
  expect(list()).toBeVisible();
});

it("claims no waiting session only when the reading supports it", () => {
  const { unmount } = render(AgentsModule, { data: null });
  expect(screen.queryByText("Nenhuma sessão aguardando você")).not.toBeInTheDocument();
  unmount();
  render(AgentsModule, { data: { ...states, failed: true, sessions: [] } as AgentsData });
  expect(screen.queryByText("Nenhuma sessão aguardando você")).not.toBeInTheDocument();
  // The box keeps its place, speaking only of the last good read.
  const time = new Intl.DateTimeFormat("pt-BR", { hour: "2-digit", minute: "2-digit" }).format(states.lastSuccessAt);
  expect(screen.getByRole("button", { name: /Nenhuma sessão aguardando na última leitura/ })).toHaveTextContent(`(${time})`);
});

it("distinguishes an unavailable reading from zero sessions", () => {
  const { container } = render(AgentsModule, { data: null });
  const area = container.querySelector(".area");
  expect(area).toHaveAttribute("data-tone", "neutral");
  expect(area).toHaveTextContent("◌ Sem leitura das sessões de agente ainda");
  expect(screen.queryByText("Nenhuma sessão de agente ativa")).not.toBeInTheDocument();
  // The area takes the place of the ring, the legend and the notice.
  expect(screen.queryByRole("button", { name: "Sessões de agente" })).not.toBeInTheDocument();
  expect(container.querySelector(".legend-list, .waiting")).toBeNull();
});

it("fills the place of the ring with an amber area when the first read failed", () => {
  const { container } = render(AgentsModule, { data: { ...empty, failed: true, lastSuccessAt: null } as AgentsData });
  const area = container.querySelector(".area");
  expect(area).toHaveAttribute("data-tone", "warn");
  expect(area).toHaveTextContent("⚠ Não foi possível consultar as sessões de agente");
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
});

it("dims the sessions of the last good read under the failure line", () => {
  const { container } = render(AgentsModule, { data: { ...states, failed: true, lastSuccessAt: 1800000000000 } as AgentsData });
  expect(container.querySelector(".area")).toBeNull();
  expect(container.querySelector(".readings")).toHaveAttribute("data-stale", "true");
  expect(ring()).toHaveTextContent("1");
});

it("summarizes the sessions as in prototype B and alerts the ones waiting for you", async () => {
  render(AgentsModule, { data: states as AgentsData });
  expect(ring()).toHaveTextContent("1");
  expect(screen.getByRole("heading", { level: 3 })).toHaveTextContent("Em andamento");
  expect(screen.queryByText("Trabalhando")).not.toBeInTheDocument();
  expect(screen.getByText("6 sessões · inclui encerradas")).toBeInTheDocument();
  const alert = screen.getByRole("button", { name: /1 aguardando você/ });
  expect(alert).toHaveTextContent("⚠ 1 aguardando você");
  expect(alert).toHaveTextContent("Ver sessões ↗");
  await userEvent.click(alert);
  expect(row("example-session")).toHaveAttribute("aria-current", "true");
});

it("shows project, name, provider, state, reason and duration of each session", async () => {
  render(AgentsModule, { data: states as AgentsData });
  await userEvent.click(screen.getByRole("button", { name: "Sessões de agente" }));
  const first = row("example-session");
  expect(first).toHaveTextContent("example-project · example-session");
  for (const text of ["Claude Code", "Aguardando você", "Aguardando aprovação", "1 min 40 s"])
    expect(within(first).getByText(text)).toBeInTheDocument();
  const codex = row("Sessão Codex");
  expect(codex).toHaveTextContent("kvojps-monitor · Sessão Codex");
  for (const text of ["Codex", "Em andamento", "1 h 01 min"])
    expect(within(codex).getByText(text)).toBeInTheDocument();
  expect(row("Sem nome")).toHaveTextContent("notes · Sem nome");
});

it("names each of the five session states in text", () => {
  render(AgentsModule, { data: states as AgentsData, notified: "s-closed" });
  for (const [name, state] of [["example-session", "Aguardando você"], ["Sessão Codex", "Em andamento"],
    ["Sem nome", "Turno concluído"], ["deploy", "Com falha"], ["Sessão encerrada", "Encerrada"]])
    expect(within(row(name)).getByText(state)).toBeInTheDocument();
  expect(within(row("Sessão encerrada")).getByText("Última situação: Com falha")).toBeInTheDocument();
});

it("summarizes retained closed sessions while keeping the detail closed", () => {
  const { container } = render(AgentsModule, { data: states as AgentsData });
  expect(container.querySelector("details")).toBeNull();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(counter("Em andamento")).toBe("1");
  expect(counter("Aguardando você")).toBe("1");
  expect(counter("Turno concluído")).toBe("1");
  expect(counter("Com falha")).toBe("1");
  expect(counter("Encerrada")).toBe("2");
});

it("derives the counters from the snapshot, without fixed values", () => {
  render(AgentsModule, { data: waiting as AgentsData });
  expect(counter("Aguardando você")).toBe("1");
  expect(counter("Em andamento")).toBe("0");
  expect(counter("Encerrada")).toBe("0");
});

it("identifies each provider and explains unavailable Codex waiting detection", async () => {
  render(AgentsModule, { data: mixed as AgentsData });
  await userEvent.click(screen.getByRole("button", { name: "Sessões de agente" }));
  expect(within(row("Sessão Codex")).getByText("Codex")).toBeInTheDocument();
  expect(within(row("Minha sessão")).getByText("Claude Code")).toBeInTheDocument();
  expect(screen.queryByText(/aguardando você/)).not.toBeInTheDocument();
});

it("says in the notice box that Codex waiting is unavailable, instead of leaving it empty", async () => {
  const configure = vi.fn();
  const { container } = render(AgentsModule, { data: mixed as AgentsData, configure });
  const box = container.querySelector(".waiting")!;
  expect(box).toHaveTextContent("Aguardando indisponível sem Detecção instantânea · Configurar Ver sessões ↗");
  expect(container.querySelector(".line")).toBeNull();
  await userEvent.click(within(box as HTMLElement).getByRole("button", { name: "Configurar" }));
  expect(configure).toHaveBeenCalledOnce();
  await userEvent.click(within(box as HTMLElement).getByRole("button", { name: "Ver sessões ↗" }));
  expect(list()).toBeVisible();
});

it("keeps the Codex detection line on top when a waiting session takes the box", () => {
  const data = { ...states, codexWaitingUnavailable: true } as AgentsData;
  render(AgentsModule, { data });
  expect(screen.getByText("Aguardando indisponível sem Detecção instantânea")).toHaveClass("line");
  expect(screen.getByRole("button", { name: /1 aguardando você/ })).toBeInTheDocument();
});

it("keeps the sessions with the age of the last good read when collection fails", async () => {
  render(AgentsModule, { data: { ...states, failed: true, observedAt: 1800000120000, lastSuccessAt: 1800000000000 } as AgentsData });
  const time = new Intl.DateTimeFormat("pt-BR", { hour: "2-digit", minute: "2-digit" }).format(1800000000000);
  expect(screen.getByText(/Não foi possível consultar/)).toHaveTextContent(`Não foi possível consultar as sessões de agente · última leitura às ${time}`);
  await userEvent.click(screen.getByRole("button", { name: "Sessões de agente" }));
  expect(row("example-session")).toBeInTheDocument();
});

it("reaches and highlights the session targeted by a notification", async () => {
  const scroll = vi.mocked(Element.prototype.scrollIntoView);
  scroll.mockClear();
  render(AgentsModule, { data: states as AgentsData, notified: "s-failed" });
  const targeted = row("deploy");
  expect(targeted).toHaveAttribute("aria-current", "true");
  expect(within(targeted).getByText("Da notificação")).toBeInTheDocument();
  expect(row("example-session")).not.toHaveAttribute("aria-current");
  await vi.waitFor(() => expect(scroll.mock.contexts).toContain(targeted));
});

it("reaches a targeted session that has closed since the notification", () => {
  render(AgentsModule, { data: states as AgentsData, notified: "s-closed" });
  expect(row("Sessão encerrada")).toHaveAttribute("aria-current", "true");
  expect(row("Outra encerrada")).not.toHaveAttribute("aria-current");
});

it("names the tool awaiting approval only when it is a known tool, never free-form detail", async () => {
  const session = states.sessions[0];
  render(AgentsModule, { data: { ...states, sessions: [
    { ...session, id: "a", name: "com ferramenta", waitingFor: "Bash" },
    { ...session, id: "b", name: "livre", waitingFor: "PRIVATE detail" },
  ] } as AgentsData });
  await userEvent.click(screen.getByRole("button", { name: "Sessões de agente" }));
  expect(within(row("com ferramenta")).getByText("Aguardando aprovação: Bash")).toBeInTheDocument();
  expect(within(row("livre")).getByText("Aguardando sua resposta")).toBeInTheDocument();
  expect(screen.queryByText(/PRIVATE/)).not.toBeInTheDocument();
});

it("opens the compact ring into a detail with all retained sessions and restores focus", async () => {
  render(AgentsModule, { data: states as AgentsData });
  const ring = screen.getByRole("button", { name: "Sessões de agente" });
  expect(screen.queryByRole("list")).not.toBeInTheDocument();
  await userEvent.click(ring);
  expect(screen.getByRole("dialog", { name: "Sessões de agente" })).toBeVisible();
  expect(row("Outra encerrada")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Fechar" }));
  expect(ring).toHaveFocus();
});

it("keeps focus and context on updates and explains an expired notification target", async () => {
  const view = render(AgentsModule, { data: states as AgentsData, notified: "s-closed" });
  const close = screen.getByRole("button", { name: "Fechar" });
  expect(close).toHaveFocus();
  await view.rerender({ data: { ...states, closed: [] } as AgentsData, notified: "s-closed" });
  expect(close).toHaveFocus();
  expect(screen.getByRole("status")).toHaveTextContent("Sessão da notificação indisponível");
  expect(screen.queryByRole("listitem", { current: true })).not.toBeInTheDocument();
});

it("reopens the preserved selection and reaches a repeated notification without stealing focus on refresh", async () => {
  const view = render(AgentsModule, { data: states as AgentsData, notified: "s-failed", notificationRequest: 1 });
  await userEvent.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Sessões de agente" }));
  expect(row("deploy")).toHaveAttribute("aria-current", "true");
  const close = screen.getByRole("button", { name: "Fechar" });
  close.focus();
  await view.rerender({ data: { ...states, observedAt: states.observedAt + 1000 } as AgentsData });
  expect(close).toHaveFocus();
  await userEvent.keyboard("{Escape}");
  await view.rerender({ notificationRequest: 2 });
  expect(screen.getByRole("dialog")).toBeVisible();
  expect(row("deploy")).toHaveAttribute("aria-current", "true");
});

it("exposes project and name from the keyboard legend and opens its matching session", async () => {
  render(AgentsModule, { data: states as AgentsData });
  const failed = legend("Com falha");
  failed.focus();
  expect(failed).toHaveAccessibleDescription("infra · deploy");
  await userEvent.keyboard("{Enter}");
  expect(row("deploy")).toHaveAttribute("aria-current", "true");
  await userEvent.keyboard("{Escape}");
  expect(failed).toHaveFocus();
});

it("reaches a notification target that arrives in a later snapshot without moving focus", async () => {
  const scroll = vi.mocked(Element.prototype.scrollIntoView);
  const view = render(AgentsModule, { data: empty as AgentsData, notified: "s-failed" });
  const close = screen.getByRole("button", { name: "Fechar" });
  scroll.mockClear();
  await view.rerender({ data: states as AgentsData });
  const target = row("deploy");
  await vi.waitFor(() => expect(scroll.mock.contexts).toContain(target));
  expect(close).toHaveFocus();
});

it("labels partial native discovery instead of implying complete session coverage", () => {
  render(AgentsModule, { data: { ...empty, discoveryPartial: true } });
  expect(screen.getByText("Descoberta de sessões ainda incompleta nesta versão Windows.")).toBeVisible();
});
