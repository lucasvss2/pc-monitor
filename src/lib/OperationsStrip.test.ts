import { tick } from "svelte";
import { act, render, screen } from "@testing-library/svelte";
import { afterEach, expect, it, vi } from "vitest";
import OperationsStrip from "./OperationsStrip.svelte";
import type { Snapshot } from "./core";

const snapshot = (id: number, state: string, origin = "user"): Snapshot => ({ module: "github", updatedAt: null, data: { operation: { id, state, origin }, title: "SECRET" } });
afterEach(() => vi.useRealTimers());

it("keeps a requested operation until its actual result and expires only the result", async () => {
  vi.useFakeTimers();
  const { rerender } = render(OperationsStrip, { snapshots: {}, ready: true });
  await rerender({ snapshots: { github: snapshot(1, "running") }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("GitHub · Estabelecendo enlace…");
  await act(() => vi.advanceTimersByTime(60_000));
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Estabelecendo enlace");
  await rerender({ snapshots: { github: snapshot(1, "succeeded") }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("GitHub · Telemetria atualizada.");
  expect(screen.getByLabelText("Faixa de bordo")).not.toHaveTextContent("SECRET");
  await act(() => vi.advanceTimersByTime(3999));
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Telemetria atualizada");
  await act(() => vi.advanceTimersByTime(1));
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
});

it("establishes an initial reference and never replays repeated snapshots or timestamps", async () => {
  vi.useFakeTimers();
  const existing = snapshot(10, "failed", "automatic");
  const { rerender } = render(OperationsStrip, { snapshots: { github: existing }, ready: false });
  await rerender({ snapshots: { github: existing }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  expect(screen.getByRole("status").textContent).toBe("");
  await rerender({ snapshots: { github: { ...existing, updatedAt: 99999 } }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  await rerender({ snapshots: { github: snapshot(11, "recovered", "automatic") }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Atualização restabelecida");
  await act(() => vi.advanceTimersByTime(4000));
  await rerender({ snapshots: { github: snapshot(11, "recovered", "automatic") }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
});

it("keeps automatic refreshes silent, shows resume visually, and announces its real failure", async () => {
  const { rerender } = render(OperationsStrip, { snapshots: {}, ready: true });
  for (const state of ["running", "succeeded"]) {
    await rerender({ snapshots: { github: snapshot(1, state, "automatic") }, ready: true });
    expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
    expect(screen.getByRole("status").textContent).toBe("");
  }
  await rerender({ snapshots: { github: snapshot(2, "running", "resume") }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Retomando atualização");
  expect(screen.getByRole("status").textContent).toBe("");
  await rerender({ snapshots: { github: snapshot(2, "failed", "resume") }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Falha ao atualizar");
});

it("does not infer a result from refreshing or updatedAt and does not extend repeated results", async () => {
  vi.useFakeTimers();
  const { rerender } = render(OperationsStrip, { snapshots: {}, ready: true });
  await rerender({ snapshots: { github: snapshot(1, "running") }, ready: true });
  await rerender({ snapshots: { github: { ...snapshot(1, "running"), data: { operation: { id: 1, origin: "user", state: "running" }, refreshing: false }, updatedAt: 123 } }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Estabelecendo enlace");
  await rerender({ snapshots: { github: snapshot(1, "cancelled") }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Operação cancelada");
  expect(screen.getByLabelText("Faixa de bordo")).not.toHaveTextContent("Telemetria atualizada");
  await act(() => vi.advanceTimersByTime(3000));
  await rerender({ snapshots: { github: snapshot(1, "cancelled") }, ready: true });
  await act(() => vi.advanceTimersByTime(1000));
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
});

it("groups six simultaneous sources, prioritizes failure and pause, and sanitizes announcements", async () => {
  const { default: fixture } = await import("./fixtures/operations/simultaneous.json");
  const { rerender } = render(OperationsStrip, { snapshots: {}, ready: true });
  const snapshots = Object.fromEntries(fixture.map(s => [s.module, s]));
  await rerender({ snapshots, ready: true });
  const strip = screen.getByLabelText("Faixa de bordo");
  expect(strip).toHaveTextContent("GitHub · Falha ao atualizar. Confira o Módulo.Tarefas · Atualização pausada. Confira o Módulo.Agentes · Atenção. Confira o Módulo. +3 Módulos");
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Sistema · Atualização restabelecida");
  expect(screen.getByRole("status")).not.toHaveTextContent("Estabelecendo enlace");
  expect(document.body).not.toHaveTextContent("SENTINEL");
});

it("announces identical results of distinct requested operations again", async () => {
  const { rerender } = render(OperationsStrip, { snapshots: {}, ready: true });
  await rerender({ snapshots: { github: snapshot(1, "succeeded") }, ready: true });
  await act(() => tick());
  const live = screen.getByRole("status");
  const delivered: string[] = [];
  const observer = new MutationObserver(() => delivered.push(live.textContent ?? ""));
  observer.observe(live, { subtree: true, childList: true, characterData: true });
  await rerender({ snapshots: { github: snapshot(2, "succeeded") }, ready: true });
  await act(() => tick());
  observer.disconnect();
  expect(delivered).toContain("GitHub · Telemetria atualizada.");
});

it("keeps concurrent AI providers independent before grouping their summaries", async () => {
  vi.useFakeTimers();
  const { rerender } = render(OperationsStrip, { snapshots: {}, ready: true });
  const { default: transitions } = await import("./fixtures/operations/ai-transitions.json");
  const first: Snapshot = { ...transitions[0], module: "ai" };
  const second: Snapshot = { ...transitions[1], module: "ai" };
  await rerender({ snapshots: { ai: first }, ready: true });
  const strip = screen.getByLabelText("Faixa de bordo");
  expect(strip).toHaveTextContent("Claude · Falha ao atualizar");
  expect(strip).toHaveTextContent("Codex · Estabelecendo enlace");
  await rerender({ snapshots: { ai: second }, ready: true });
  expect(strip).toHaveTextContent("Claude · Falha ao atualizar");
  expect(strip).toHaveTextContent("Codex · Telemetria atualizada");
  expect(document.body).not.toHaveTextContent("SENTINEL");
  await act(() => vi.advanceTimersByTime(4000));
  expect(strip).toHaveTextContent("Bordo em repouso");
});


it("uses AI provider identities for recovery, pause, resume, repetition and reopening", async () => {
  vi.useFakeTimers();
  const { default: transitions } = await import("./fixtures/operations/ai-transitions.json");
  const state = (index: number): Snapshot => ({ ...transitions[index], module: "ai" });
  const { rerender, unmount } = render(OperationsStrip, { snapshots: {}, ready: true });
  await rerender({ snapshots: { ai: state(2) }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Claude · Atualização restabelecida");
  expect(screen.getByRole("status")).not.toHaveTextContent("Codex");
  await rerender({ snapshots: { ai: state(3) }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Codex · Atualização pausada");
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Claude · Atualização pausada");
  await act(() => vi.advanceTimersByTime(3000));
  await rerender({ snapshots: { ai: state(3) }, ready: true });
  await act(() => vi.advanceTimersByTime(1000));
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  await rerender({ snapshots: { ai: state(4) }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Codex · Retomando atualização");
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Claude · Retomando atualização");
  await act(() => vi.advanceTimersByTime(60_000));
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Retomando atualização");
  // A discarded request has no fabricated result or accessible announcement.
  await rerender({ snapshots: { ai: { module: "ai", updatedAt: null, data: { codex: {}, claude: {} } } }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  expect(screen.getByRole("status")).not.toHaveTextContent(/Telemetria|cancelada/);
  unmount();
  render(OperationsStrip, { snapshots: { ai: state(2) }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  expect(screen.getByRole("status").textContent).toBe("");
});

it("groups Trello movement and refresh independently and announces sanitized actual results", async () => {
  vi.useFakeTimers();
  const { rerender } = render(OperationsStrip, { snapshots: {}, ready: true });
  const { default: transitions } = await import("./fixtures/operations/trello-transitions.json");
  const state = (index: number): Snapshot => ({ module: "tasks", updatedAt: 1, data: {
    ...transitions[index], card: "CARD_SENTINEL", list: "LIST_SENTINEL", token: "TOKEN_SENTINEL",
  } });
  await rerender({ snapshots: { tasks: state(0) }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Alterando tarefa…");
  await rerender({ snapshots: { tasks: state(1) }, ready: true });
  await act(() => tick());
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Falha ao atualizar");
  expect(screen.getByRole("status")).toHaveTextContent("Alteração confirmada.");
  expect(screen.getByRole("status")).not.toHaveTextContent("SENTINEL");
  await act(() => vi.advanceTimersByTime(4000));
  await rerender({ snapshots: { tasks: state(1) }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
});

it("shows fast Trello failures recovery and cancellation and establishes a fresh reference on reopen", async () => {
  vi.useFakeTimers();
  const { default: transitions } = await import("./fixtures/operations/trello-transitions.json");
  const snapshot = (index: number): Snapshot => ({ module: "tasks", updatedAt: null, data: transitions[index] });
  const { rerender, unmount } = render(OperationsStrip, { snapshots: {}, ready: true });
  await rerender({ snapshots: { tasks: snapshot(2) }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Falha ao mover card");
  expect(screen.getByRole("status")).not.toHaveTextContent("Retomando");
  await rerender({ snapshots: { tasks: snapshot(3) }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Atualização restabelecida");
  expect(screen.getByRole("status")).toHaveTextContent("Movimentação cancelada");
  await act(() => vi.advanceTimersByTime(4000));
  await rerender({ snapshots: { tasks: snapshot(4) }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  unmount();
  render(OperationsStrip, { snapshots: { tasks: snapshot(3) }, ready: true });
  expect(screen.getByRole("status").textContent).toBe("");
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
});

it("communicates network categories without replaying samples or exposing source content", async () => {
  vi.useFakeTimers();
  const { default: good } = await import("./network/fixtures/good.json");
  const baseline: Snapshot = { ...good, module: "network" };
  const { rerender } = render(OperationsStrip, { snapshots: { network: baseline }, ready: true });
  const changed: Snapshot = { ...baseline, data: { ...good.data, vpn: true, quality: "poor", security: "risk", reasons: ["SENTINEL"] } };
  await rerender({ snapshots: { network: changed }, ready: true });
  await act(() => tick());
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("VPN conectada");
  expect(screen.getByRole("status")).toHaveTextContent("Qualidade: Ruim");
  expect(screen.getByRole("status")).toHaveTextContent("Segurança: Risco");
  expect(document.body).not.toHaveTextContent("SENTINEL");
  await act(() => vi.advanceTimersByTime(4000));
  await rerender({ snapshots: { network: { ...changed, updatedAt: 99999 } }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
});

it("tracks real speed outcomes, keeps long tests active, and silences automatic starts only", async () => {
  vi.useFakeTimers();
  const { default: transitions } = await import("./fixtures/operations/network-speed-transitions.json");
  const state = (index: number): Snapshot => ({ ...transitions[index], module: "network" });
  const { rerender } = render(OperationsStrip, { snapshots: {}, ready: true });
  await rerender({ snapshots: { network: state(0) }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Testando velocidade");
  expect(screen.getByRole("status").textContent).toBe("");
  await act(() => vi.advanceTimersByTime(120_000));
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Testando velocidade");
  await rerender({ snapshots: { network: state(1) }, ready: true });
  await act(() => tick());
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Teste de velocidade concluído");
  expect(screen.getByRole("status")).not.toHaveTextContent("Teste de velocidade concluído");
  // A fast operation can deliver its real failure without an observed start.
  await rerender({ snapshots: { network: state(3) }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Falha no teste de velocidade");
  await rerender({ snapshots: { network: state(6) }, ready: true });
  await act(() => vi.advanceTimersByTime(60_000));
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Testando velocidade");
  await rerender({ snapshots: { network: state(7) }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Teste de velocidade cancelado");
  expect(screen.getByLabelText("Faixa de bordo")).not.toHaveTextContent("concluído");
  await act(() => vi.advanceTimersByTime(4000));
  await rerender({ snapshots: { network: state(7) }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
});

it("baselines connection VPN and categories on actual reopening and ignores continuous readings", async () => {
  vi.useFakeTimers();
  const { default: good } = await import("./network/fixtures/good.json");
  const data = { ...good.data, connected: true, vpn: false };
  const state = (patch = {}): Snapshot => ({ module: "network", updatedAt: 1, data: { ...data, ...patch } });
  const { rerender } = render(OperationsStrip, { snapshots: { network: state() }, ready: true });
  await rerender({ snapshots: { network: state({ internet: { latency: 999 }, traffic: { download: 999 }, networkId: "SENTINEL" }) }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  await rerender({ snapshots: { network: state({ connected: false, vpn: true, quality: null, security: null }) }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Conexão perdida");
  expect(screen.getByRole("status")).toHaveTextContent("Qualidade indisponível");
  await rerender({ snapshots: { network: state() }, ready: false });
  await rerender({ snapshots: { network: state() }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  expect(screen.getByRole("status").textContent).toBe("");
  await rerender({ snapshots: { network: state({ vpn: true, quality: "regular", security: "warning" }) }, ready: true });
  await rerender({ snapshots: { network: state() }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("VPN desconectada");
  expect(screen.getByRole("status")).toHaveTextContent("Qualidade: Boa");
  expect(screen.getByRole("status")).toHaveTextContent("Segurança: Sem alertas");
  expect(document.body).not.toHaveTextContent("SENTINEL");
});

it("announces a real connection revision while remaining connected and ignores unchanged revisions", async () => {
  vi.useFakeTimers();
  const { default: good } = await import("./network/fixtures/good.json");
  const initial: Snapshot = { ...good, module: "network" };
  const { rerender } = render(OperationsStrip, { snapshots: { network: initial }, ready: true });
  const switched: Snapshot = { ...initial, data: { ...good.data, connectionRevision: 1 } };
  await rerender({ snapshots: { network: switched }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Conexão alterada");
  await act(() => vi.advanceTimersByTime(4000));
  await rerender({ snapshots: { network: { ...switched, updatedAt: 2000 } }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  await rerender({ snapshots: { network: { ...initial, data: { ...good.data, connected: false, connectionRevision: 1 } } }, ready: true });
  await rerender({ snapshots: { network: switched }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Conexão estabelecida");
  expect(screen.getByRole("status")).not.toHaveTextContent("Conexão alterada");
});

it("groups actual agent transitions without content or inferred turns", async () => {
  vi.useFakeTimers();
  const { default: fixture } = await import("./agents/fixtures/states.json");
  const state = (working: boolean): Snapshot => ({ module: "agents", updatedAt: null, data: {
    ...fixture, sessions: fixture.sessions.map(session => ({ ...session,
      state: working ? "working" : session.state, interrupted: !working && session.state === "done", name: "NAME_SENTINEL", project: "PROJECT_SENTINEL",
      branch: "BRANCH_SENTINEL", waitingFor: "PROMPT_SENTINEL",
    })),
  } });
  const { rerender } = render(OperationsStrip, { snapshots: { agents: state(true) }, ready: true });
  await rerender({ snapshots: { agents: state(false) }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Aguardando você: 1");
  expect(screen.getByRole("status")).toHaveTextContent("Turno concluído: 1");
  expect(screen.getByRole("status")).toHaveTextContent("Com falha: 1");
  expect(document.body).not.toHaveTextContent("SENTINEL");
  await act(() => vi.advanceTimersByTime(4000));
  await rerender({ snapshots: { agents: state(false) }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
});

it("reports closure only when observed and resets agent references on reopening", async () => {
  const { default: fixture } = await import("./agents/fixtures/states.json");
  const initial: Snapshot = { module: "agents", updatedAt: 1, data: fixture };
  const closed: Snapshot = { ...initial, data: { ...fixture, sessions: [], closed: [
    ...fixture.closed, ...fixture.sessions.map(session => ({ ...session, state: "closed" })),
  ] } };
  const { rerender } = render(OperationsStrip, { snapshots: { agents: initial }, ready: true });
  await rerender({ snapshots: { agents: closed }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Encerrada: 4");
  expect(screen.getByRole("status")).not.toHaveTextContent("Turno concluído");
  await rerender({ snapshots: { agents: closed }, ready: false });
  await rerender({ snapshots: { agents: closed }, ready: true });
  expect(screen.getByRole("status").textContent).toBe("");
});

it("reports existing system alert edges and never treats missing sensors as recovery", async () => {
  vi.useFakeTimers();
  const { default: warning } = await import("./system/fixtures/warning.json");
  const { default: desktop } = await import("./system/fixtures/desktop.json");
  const sample = (data: unknown): Snapshot => ({ module: "system", updatedAt: 1, data });
  const normal = { ...warning.data,
    cpu: { ...warning.data.cpu, temperature: { celsius: 40, warning: false } },
    disks: warning.data.disks.map(disk => ({ ...disk, warning: false })),
    battery: { ...warning.data.battery, warning: false },
  };
  const { rerender } = render(OperationsStrip, { snapshots: { system: sample(normal) }, ready: true });
  await rerender({ snapshots: { system: warning as Snapshot }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Temperatura alta");
  expect(screen.getByRole("status")).toHaveTextContent("Disco com pouco espaço");
  expect(screen.getByRole("status")).toHaveTextContent("Bateria baixa");
  await act(() => vi.advanceTimersByTime(4000));
  await rerender({ snapshots: { system: warning as Snapshot }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  await rerender({ snapshots: { system: sample(normal) }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Alerta de temperatura encerrado");
  expect(screen.getByRole("status")).toHaveTextContent("Alerta de disco encerrado");
  expect(screen.getByRole("status")).toHaveTextContent("Alerta de bateria encerrado");
  await rerender({ snapshots: { system: warning as Snapshot }, ready: false });
  await rerender({ snapshots: { system: warning as Snapshot }, ready: true });
  await rerender({ snapshots: { system: sample({ ...desktop.data, cpu: { ...desktop.data.cpu, temperature: null }, disks: [], battery: null }) }, ready: true });
  expect(screen.getByRole("status").textContent).toBe("");
});


it("announces a real system alert when a missing sensor returns already warning", async () => {
  const { default: fixture } = await import("./system/fixtures/warning.json");
  const missing: Snapshot = { module: "system", updatedAt: null, data: {
    ...fixture.data, cpu: { ...fixture.data.cpu, temperature: null }, battery: null, disks: [],
  } };
  const { rerender } = render(OperationsStrip, { snapshots: { system: missing }, ready: true });
  await rerender({ snapshots: { system: fixture as Snapshot }, ready: true });
  await act(() => tick());
  expect(screen.getByRole("status")).toHaveTextContent("Temperatura alta");
  expect(screen.getByRole("status")).toHaveTextContent("Bateria baixa");
});

it("baselines delayed first readings without replaying existing agent states or system alerts", async () => {
  const { default: agents } = await import("./agents/fixtures/states.json");
  const { default: system } = await import("./system/fixtures/warning.json");
  const { rerender } = render(OperationsStrip, { snapshots: {}, ready: true });
  await rerender({ snapshots: { agents: { module: "agents", updatedAt: null, data: agents }, system: system as Snapshot }, ready: true });
  expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
  expect(screen.getByRole("status").textContent).toBe("");
});
