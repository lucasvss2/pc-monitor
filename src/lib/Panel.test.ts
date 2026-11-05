import { act, render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import Panel from "./Panel.svelte";
import type { Module, Core, Snapshot, InitialState, PanelMode, Presentation } from "./core";
import networkSpeed from "./network/fixtures/speed.json";
import networkFull from "./network/fixtures/full.json";
import notebook from "./system/fixtures/notebook.json";
import waitingAgents from "./agents/fixtures/waiting.json";
import aiFull from "./ai/fixtures/full.json";
import githubFull from "./github/fixtures/full.json";
import trelloFull from "./trello/fixtures/full.json";

/** Stands in for the webview IntersectionObserver: tests decide what intersects. */
class FakeObserver {
  static instances: FakeObserver[] = [];
  targets = new Set<Element>();
  constructor(public callback: IntersectionObserverCallback) {
    FakeObserver.instances.push(this);
  }
  observe(target: Element) {
    this.targets.add(target);
  }
  unobserve(target: Element) {
    this.targets.delete(target);
  }
  disconnect() {
    this.targets.clear();
  }
}

/** Simulate scrolling so that exactly these modules intersect the visible area. */
async function intersect(modules: Module[]) {
  await act(() => {
    for (const observer of FakeObserver.instances) {
      const entries = Array.from(observer.targets).map((target) => ({
        target,
        isIntersecting: modules.some((m) => target.id === `module-${m}`),
      }));
      observer.callback(entries as unknown as IntersectionObserverEntry[], observer as unknown as IntersectionObserver);
    }
  });
}

beforeEach(() => {
  // Keep snapshot windows valid independently of the test execution date.
  vi.spyOn(Date, "now").mockReturnValue(1793460871000);
  FakeObserver.instances = [];
  vi.stubGlobal("IntersectionObserver", FakeObserver);
});
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });

function fakeCore(initial: { runtimeId?: string; lastAnchor?: Module; snapshots?: Snapshot[]; agentTarget?: string; unavailableModules?: Module[] } = {}) {
  let listener: ((s: Snapshot) => void) | undefined;
  const core = {
    initialState: vi.fn(async (): Promise<InitialState> => ({
      runtimeId: initial.runtimeId,
      unavailableModules: initial.unavailableModules,
      lastAnchor: initial.lastAnchor ?? "ai",
      agentTarget: initial.agentTarget ?? null,
      snapshots: initial.snapshots ?? [],
      panelVisible: true,
    })),
    listenSnapshots: vi.fn(async (cb: (s: Snapshot) => void) => {
      listener = cb;
      return () => {};
    }),
    reportVisibleModules: vi.fn(),
    chooseAnchor: vi.fn(),
    closePanel: vi.fn(),
  } satisfies Core;
  return {
    core,
    publish: (s: Snapshot) => act(() => listener?.(s)),
    publishAll: (snapshots: Snapshot[]) => act(() => { snapshots.forEach(s => listener?.(s)); }),
  };
}

// The outer module section comes first; some modules label an inner region too.
const section = (name: string) => screen.getAllByRole("region", { name })[0];
const settingsButton = () => screen.getByRole("button", { name: "Configurações" });

describe("Compact panel", () => {
  it("keeps pin and the local clock while expanding a summary temporarily", async () => {
    const { core } = fakeCore();
    let view: Presentation = { mode: "full", preferredMode: "full", pinned: false, pinAvailable: true, pinFailed: false };
    const setPanelMode = vi.fn(async (mode: PanelMode, remember: boolean) => {
      view = { ...view, mode, preferredMode: remember ? mode : view.preferredMode };
      return view;
    });
    render(Panel, { core: {
      ...core,
      initialState: async () => ({ ...await core.initialState(), presentation: view }),
      setPanelMode,
      setPanelPinned: async (pinned: boolean) => (view = { ...view, pinned }),
      panelClock: async () => Date.UTC(2026, 10, 5, 0, 0, 8),
    } });
    const user = userEvent.setup();
    const clock = await screen.findByRole("group", { name: "Relógio do Painel" });
    await screen.findByText("00:00:08 · 05/11/2026");
    await user.click(await screen.findByRole("button", { name: "Manter no topo" }));
    await user.click(screen.getByRole("button", { name: "Reduzir Painel" }));
    expect(await screen.findByRole("button", { name: "Expandir Painel" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Manter no topo" })).toHaveAttribute("aria-pressed", "true");
    expect(clock).toBeVisible();
    await user.click(within(screen.getByRole("region", { name: "Tarefas" })).getByRole("button"));
    await user.click(await screen.findByRole("button", { name: "Voltar ao reduzido" }));
    expect(await screen.findByRole("button", { name: "Expandir Painel" })).toBeInTheDocument();
    expect(setPanelMode.mock.calls).toEqual([["compact", true], ["full", false], ["compact", false]]);
    expect(clock).toHaveTextContent("00:00:08 · 05/11/2026");
  });

  it("shows six independent modules with IA and Agents together and no redundant navigation", async () => {
    const { core } = fakeCore();
    render(Panel, { core });
    await screen.findByRole("region", { name: "IA" });
    expect(section("IA").parentElement).toBe(section("Agentes").parentElement);
    expect(Array.from(document.querySelectorAll("section[data-module]")).map((s) => s.id))
      .toEqual(["module-ai", "module-agents", "module-github", "module-network", "module-tasks", "module-system"]);
    expect(screen.queryByRole("navigation", { name: "Módulos" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { level: 1 })).not.toBeInTheDocument();
    expect(settingsButton()).toBeInTheDocument();
  });

  it("frames the panel as prototype B: top bar, settings on the right, one IA e Agentes card", async () => {
    const fourteenThirty = new Date(2026, 9, 30, 14, 32).getTime();
    const { core } = fakeCore({ snapshots: [{ module: "ai", updatedAt: fourteenThirty, data: null }] });
    const { container } = render(Panel, { core });

    const ai = await screen.findByRole("region", { name: "IA" });
    expect(container.querySelectorAll(".bridge span")).toHaveLength(3);
    expect(settingsButton()).toHaveTextContent("⚙ Configurações");
    expect(screen.queryByText("Demonstração · leituras independentes")).not.toBeInTheDocument();
    expect(screen.getAllByRole("heading", { name: "IA e Agentes" })).toHaveLength(1);
    expect(ai.closest(".card")).toBe(section("Agentes").closest(".card"));
    expect(within(ai).queryByText("Codex e Claude Code")).not.toBeInTheDocument();
    expect(within(ai).queryByText("atualizado 14:32")).not.toBeInTheDocument();
    expect(within(section("Agentes")).queryByRole("heading", { level: 2 })).not.toBeInTheDocument();
    expect(container.querySelector(".code")).toBeNull();
    expect(within(section("Rede")).queryByText("NetworkManager e kernel")).not.toBeInTheDocument();
    expect(within(section("Rede")).getByRole("heading", { level: 2, name: "Rede" })).toBeInTheDocument();
    expect(within(section("Tarefas")).getByRole("heading", { level: 2, name: "Tarefas" })).toBeInTheDocument();
    expect(within(section("Tarefas")).queryByText("sem dados ainda")).not.toBeInTheDocument();
    expect(within(section("Sistema")).getByRole("heading", { level: 2, name: "Sistema" })).toBeInTheDocument();
    expect(within(section("Sistema")).queryByText(/Kernel e UPower|atualizado/)).not.toBeInTheDocument();
  });

  it("shows cached snapshots at once and updates only the module the core pushes", async () => {
    const { core, publish } = fakeCore({ snapshots: [notebook as Snapshot] });
    render(Panel, { core });
    expect(await within(await screen.findByRole("region", { name: "Sistema" })).findByRole("group", { name: "CPU" })).toHaveTextContent("2%");

    await publish({ module: "tasks", updatedAt: new Date(2026, 9, 30, 9, 5).getTime(), data: trelloFull });

    expect(within(section("Tarefas")).getByText("17 cards · 8 listas")).toBeInTheDocument();
    expect(within(section("Sistema")).getByRole("group", { name: "CPU" })).toHaveTextContent("2%");
  });

  it("keeps keyboard focus when a snapshot arrives", async () => {
    const { core, publish } = fakeCore({ snapshots: [{ module: "ai", updatedAt: 1, data: aiFull }] });
    render(Panel, { core: { ...core, refreshAi: vi.fn() } });
    const configure = await screen.findByRole("button", { name: "Configurar IA" });
    configure.focus();

    await publish({ module: "ai", updatedAt: 2, data: aiFull });

    expect(screen.getByRole("button", { name: "Configurar IA" })).toHaveFocus();
  });
});

describe("Panel navigation", () => {
  it("restores the last anchor without a navigation rail", async () => {
    const scroll = vi.mocked(Element.prototype.scrollIntoView);
    scroll.mockClear();
    const { core } = fakeCore({ lastAnchor: "network" });
    render(Panel, { core });
    await vi.waitFor(() => expect(scroll).toHaveBeenCalled());
    expect(scroll.mock.contexts.at(-1)).toBe(section("Rede"));
    expect(core.chooseAnchor).not.toHaveBeenCalled();
  });

  it("remembers the module reached through its local actions", async () => {
    const { core } = fakeCore();
    render(Panel, { core });
    const trello = await screen.findByRole("region", { name: "Tarefas" });
    await userEvent.click(within(trello).getByRole("button", { name: "Configurar Tarefas" }));
    await userEvent.click(screen.getByRole("button", { name: "Voltar ao Painel" }));
    expect(core.chooseAnchor).toHaveBeenLastCalledWith("tasks");
    expect(within(trello).getByRole("button", { name: "Configurar Tarefas" })).toHaveFocus();
  });

  it("Escape asks the core to close the panel, also in Settings", async () => {
    const { core } = fakeCore();
    render(Panel, { core });
    await screen.findByRole("region", { name: "IA" });

    await userEvent.keyboard("{Escape}");
    await userEvent.click(settingsButton());
    await userEvent.keyboard("{Escape}");

    expect(core.closePanel).toHaveBeenCalledTimes(2);
  });
});

describe("Panel visibility", () => {
  it("reports no modules while the native panel is hidden and restores intersections on show", async () => {
    const { core } = fakeCore();
    let visibility: ((visible: boolean) => void) | undefined;
    render(Panel, { core: { ...core, listenPanelVisibility: async (cb) => { visibility = cb; return () => {}; } } });
    await screen.findByRole("region", { name: "IA" });
    await intersect(["ai", "agents"]);
    await act(() => visibility?.(false));
    expect(core.reportVisibleModules).toHaveBeenLastCalledWith([]);
    await intersect(["github"]);
    expect(core.reportVisibleModules).toHaveBeenLastCalledWith([]);
    await act(() => visibility?.(true));
    expect(core.reportVisibleModules).toHaveBeenLastCalledWith(["github"]);
  });

  it("reports every module intersecting the visible area", async () => {
    const { core } = fakeCore();
    render(Panel, { core });
    await screen.findByRole("region", { name: "IA" });

    await intersect(["ai", "agents", "github"]);
    expect(core.reportVisibleModules).toHaveBeenLastCalledWith(["ai", "agents", "github"]);

    // Scrolling down: the first row leaves the area.
    await intersect(["system", "tasks", "network"]);
    expect(core.reportVisibleModules).toHaveBeenLastCalledWith(["tasks", "network", "system"]);
  });

  it("does not report again when intersection events repeat the same set", async () => {
    const { core } = fakeCore();
    render(Panel, { core });
    await screen.findByRole("region", { name: "IA" });

    core.reportVisibleModules.mockClear();
    await intersect(["ai", "system"]);
    await intersect(["system", "ai"]);
    await intersect(["ai", "system"]);

    expect(core.reportVisibleModules).toHaveBeenCalledOnce();
  });

  it("Settings reports an empty set and returning restores the visible set and context", async () => {
    const { core } = fakeCore();
    render(Panel, { core });
    await screen.findByRole("region", { name: "IA" });
    await intersect(["tasks", "network"]);
    const opener = settingsButton();

    await userEvent.click(opener);

    expect(core.reportVisibleModules).toHaveBeenLastCalledWith([]);
    expect(screen.getByRole("heading", { name: "Configurações" })).toBeInTheDocument();
    expect(screen.queryByRole("main")).not.toBeInTheDocument();
    // Intersection events while in Settings do not count.
    await intersect(["ai"]);
    expect(core.reportVisibleModules).toHaveBeenLastCalledWith([]);

    await userEvent.click(screen.getByRole("button", { name: "Voltar ao Painel" }));

    expect(core.reportVisibleModules).toHaveBeenLastCalledWith(["ai"]);
    expect(screen.queryByRole("heading", { name: "Configurações" })).not.toBeInTheDocument();
    expect(section("Agentes")).toBeInTheDocument();
    expect(opener).toHaveFocus();
    expect(core.chooseAnchor).not.toHaveBeenCalled();
  });


});

it("sends the network automatic speed preference from Settings", async () => {
  const { core } = fakeCore({ snapshots: [networkSpeed as Snapshot] });
  const setAutomaticSpeed = vi.fn();
  render(Panel, { core: { ...core, setAutomaticSpeed } });
  await screen.findByRole("region", { name: "Rede" });
  await userEvent.click(settingsButton());
  await userEvent.click(screen.getByRole("button", { name: "Rede" }));
  const toggle = screen.getByRole("checkbox", { name: "Teste de velocidade automático" });
  expect(toggle).toBeChecked();
  await userEvent.click(toggle);
  expect(setAutomaticSpeed).toHaveBeenCalledWith(false);
});

it("sends a pasted GitHub token from Settings to the core", async () => {
  const { default: missing } = await import("./github/fixtures/missing-gh.json");
  const { core } = fakeCore({ snapshots: [{ module: "github", updatedAt: null, data: missing }] });
  const saveGithubToken = vi.fn().mockResolvedValue("exemplo");
  render(Panel, { core: { ...core, saveGithubToken } });
  await screen.findByRole("region", { name: "GitHub" });
  await userEvent.click(settingsButton());
  await userEvent.click(screen.getByRole("button", { name: "GitHub" }));
  expect(screen.getByRole("heading", { name: "Configurações" })).toBeInTheDocument();
  await userEvent.type(screen.getByLabelText("Token clássico (escopo repo)"), "private-token");
  const settings = screen.getByRole("region", { name: "GitHub" });
  await userEvent.click(within(settings).getByRole("button", { name: "Validar e salvar" }));
  expect(saveGithubToken).toHaveBeenCalledWith("private-token");
  expect(await screen.findByText("Token próprio · exemplo")).toBeInTheDocument();
});

it("sends Trello key and token from Settings to the core", async () => {
  const { default: empty } = await import("./trello/fixtures/empty.json");
  const { core } = fakeCore({ snapshots: [{ module: "tasks", updatedAt: null, data: empty }] });
  const connectTrello = vi.fn().mockResolvedValue("exemplo");
  render(Panel, { core: { ...core, connectTrello } });
  await screen.findByRole("region", { name: "Tarefas" });
  await userEvent.click(settingsButton());
  await userEvent.click(screen.getByRole("button", { name: "Tarefas" }));
  expect(core.reportVisibleModules).toHaveBeenLastCalledWith([]);
  const trello = screen.getByRole("region", { name: "Tarefas" });
  await userEvent.type(within(trello).getByLabelText("API key"), "0123abcd");
  await userEvent.type(within(trello).getByLabelText("Token"), "private-token");
  await userEvent.click(within(trello).getByRole("button", { name: "Validar e salvar" }));
  expect(connectTrello).toHaveBeenCalledWith("0123abcd", "private-token");
  expect(await within(trello).findByText("Conta exemplo · escolha um quadro")).toBeInTheDocument();
});

it("opens at Agents for a notification target without changing the last anchor", async () => {
  const scroll = vi.mocked(Element.prototype.scrollIntoView);
  const { core } = fakeCore({ lastAnchor: "agents", agentTarget: waitingAgents.sessions[0].id,
    snapshots: [{ module: "agents", updatedAt: 1000, data: waitingAgents }] });
  render(Panel, { core });
  await screen.findByRole("region", { name: "Agentes" });
  await vi.waitFor(() => expect(scroll.mock.contexts).toContain(section("Agentes")));
  expect(core.chooseAnchor).not.toHaveBeenCalled();
});

it("shows three enabled Agent notification switches and sends the changed preferences", async () => {
  const { core } = fakeCore({ snapshots: [{ module: "agents", updatedAt: 1000, data: waitingAgents }] });
  const setAgentNotifications = vi.fn();
  render(Panel, { core: { ...core, setAgentNotifications } });
  await screen.findByRole("region", { name: "IA" });
  await userEvent.click(settingsButton());
  await userEvent.click(screen.getByRole("button", { name: "Agentes" }));
  for (const name of ["Aguardando", "Turno concluído", "Falha"]) expect(screen.getByRole("checkbox", { name })).toBeChecked();
  await userEvent.click(screen.getByRole("checkbox", { name: "Aguardando" }));
  expect(setAgentNotifications).toHaveBeenCalledWith({ waiting: false, done: true, failed: true });
});

it("brings an already open panel, even in Settings, to the session targeted by a notification", async () => {
  const scroll = vi.mocked(Element.prototype.scrollIntoView);
  const { core } = fakeCore({ lastAnchor: "network", snapshots: [{ module: "agents", updatedAt: 1000, data: waitingAgents }] });
  let target: ((session: string) => void) | undefined;
  render(Panel, { core: { ...core, listenAgentTarget: async (cb: (session: string) => void) => { target = cb; return () => {}; } } });
  await screen.findByRole("region", { name: "Rede" });
  await userEvent.click(settingsButton());
  scroll.mockClear();

  await act(() => target?.(waitingAgents.sessions[0].id));

  await vi.waitFor(() => expect(scroll.mock.contexts).toContain(section("Agentes")));
  expect(screen.queryByRole("heading", { name: "Configurações" })).not.toBeInTheDocument();
  const targeted = await screen.findByRole("listitem", { current: true });
  expect(within(targeted).getByText("Minha sessão")).toBeInTheDocument();
  await vi.waitFor(() => expect(scroll.mock.contexts).toContain(targeted));
  expect(core.chooseAnchor).not.toHaveBeenCalled();
});

it("keeps focus on a module when a snapshot arrives", async () => {
  const { core, publish } = fakeCore({ snapshots: [{ module: "ai", updatedAt: 1, data: aiFull }] });
  render(Panel, { core });
  await screen.findByRole("region", { name: "IA" });
  section("IA").focus();

  await publish({ module: "ai", updatedAt: 2, data: aiFull });

  expect(section("IA")).toHaveFocus();
});

describe("AI module in the panel", () => {
  it("explains there is no reading yet and keeps its actions before the first snapshot", async () => {
    const { core } = fakeCore();
    const refreshAi = vi.fn();
    render(Panel, { core: { ...core, refreshAi } });
    const ai = await screen.findByRole("region", { name: "IA" });
    expect(within(ai).getByText("Detectando ferramentas de IA…")).toBeInTheDocument();
    expect(within(ai).getByRole("button", { name: "Configurar IA" })).toBeInTheDocument();
    await userEvent.click(within(ai).getByRole("button", { name: "Atualizar IA" }));
    expect(refreshAi).toHaveBeenCalledOnce();
  });

  it("shows the AI instruments and sends a refresh to the core", async () => {
    const { core } = fakeCore({ snapshots: [{ module: "ai", updatedAt: 1, data: aiFull }] });
    const refreshAi = vi.fn();
    render(Panel, { core: { ...core, refreshAi } });
    const ai = await screen.findByRole("region", { name: "IA" });
    expect(within(ai).getByRole("meter", { name: "Codex · janela de 5 h" })).toBeInTheDocument();
    await userEvent.click(within(ai).getByRole("button", { name: "Atualizar IA" }));
    expect(refreshAi).toHaveBeenCalledOnce();
  });

  it("opens Settings from the module and returns to it", async () => {
    const { core } = fakeCore({ snapshots: [{ module: "ai", updatedAt: 1, data: aiFull }] });
    render(Panel, { core });
    await userEvent.click(await screen.findByRole("button", { name: "Configurar IA" }));
    expect(screen.getByRole("heading", { name: "Configurações" })).toBeInTheDocument();
    expect(core.reportVisibleModules).toHaveBeenLastCalledWith([]);
    await userEvent.click(screen.getByRole("button", { name: "Voltar ao Painel" }));
    expect(screen.getByRole("button", { name: "Configurar IA" })).toHaveFocus();
  });

  it("reconnects Claude only on click and falls back to Settings when it fails", async () => {
    const { default: replaced } = await import("./ai/fixtures/claude-replaced.json");
    const { core } = fakeCore({ snapshots: [{ module: "ai", updatedAt: 1, data: replaced }] });
    const connectClaude = vi.fn().mockRejectedValue("unwritable");
    render(Panel, { core: { ...core, connectClaude } });
    const reconnect = await screen.findByRole("button", { name: "Reconectar" });
    expect(connectClaude).not.toHaveBeenCalled();
    await userEvent.click(reconnect);
    expect(connectClaude).toHaveBeenCalledOnce();
    expect(await screen.findByRole("heading", { name: "Configurações" })).toBeInTheDocument();
  });
});

describe("Agents module in the panel", () => {
  it("explains there is no reading yet before the first snapshot", async () => {
    const { core } = fakeCore();
    render(Panel, { core });
    const agents = await screen.findByRole("region", { name: "Agentes" });
    expect(within(agents).getByText("Sem leitura das sessões de agente ainda")).toBeInTheDocument();
  });

  it("shows the sessions and the intervention count from the snapshot", async () => {
    const { core } = fakeCore({ snapshots: [{ module: "agents", updatedAt: 1, data: waitingAgents }] });
    render(Panel, { core });
    const agents = await screen.findByRole("region", { name: "Agentes" });
    expect(within(agents).getByRole("button", { name: /1 aguardando você/ })).toBeInTheDocument();
    await userEvent.click(within(agents).getByRole("button", { name: "Sessões de agente" }));
    expect(within(within(agents).getByRole("list", { name: "Sessões de agente" })).getByText("Minha sessão")).toBeInTheDocument();
  });

  it("highlights the notified session when the webview is recreated with a target", async () => {
    const scroll = vi.mocked(Element.prototype.scrollIntoView);
    const { core } = fakeCore({ lastAnchor: "network", agentTarget: waitingAgents.sessions[0].id,
      snapshots: [{ module: "agents", updatedAt: 1000, data: waitingAgents }] });
    render(Panel, { core });
    const targeted = await screen.findByRole("listitem", { current: true });
    expect(within(targeted).getByText("Da notificação")).toBeInTheDocument();
    await vi.waitFor(() => expect(scroll.mock.contexts).toContain(targeted));
    expect(section("Agentes")).toBeInTheDocument();
    expect(core.chooseAnchor).not.toHaveBeenCalled();
  });

  it("drops the notification highlight once the user chooses a module by keyboard", async () => {
    const { core } = fakeCore({ agentTarget: waitingAgents.sessions[0].id,
      snapshots: [{ module: "agents", updatedAt: 1000, data: waitingAgents }] });
    render(Panel, { core });
    await screen.findByRole("listitem", { current: true });
    await userEvent.keyboard("{Escape}");
    section("Agentes").focus();
    await userEvent.keyboard("{Enter}");
    expect(section("Agentes")).toHaveFocus();
    expect(core.chooseAnchor).toHaveBeenLastCalledWith("agents");
    expect(screen.queryByRole("listitem", { current: true })).not.toBeInTheDocument();
  });
});

describe("GitHub module in the panel", () => {
  it("explains there is no reading yet and sends a refresh to the core", async () => {
    const { core } = fakeCore();
    const refreshGithub = vi.fn();
    render(Panel, { core: { ...core, refreshGithub } });
    const github = await screen.findByRole("region", { name: "GitHub" });
    expect(within(github).getByText("Sem leitura da atividade do GitHub ainda")).toBeInTheDocument();
    await userEvent.click(within(github).getByRole("button", { name: "Atualizar GitHub" }));
    expect(refreshGithub).toHaveBeenCalledOnce();
  });

  it("shows the activity and featured repository from the snapshot", async () => {
    const { core } = fakeCore({ snapshots: [{ module: "github", updatedAt: 1, data: githubFull }] });
    render(Panel, { core });
    const github = await screen.findByRole("region", { name: "GitHub" });
    expect(within(github).getByRole("region", { name: "Repo em destaque" })).toHaveTextContent("exemplo/repo-1");
    expect(within(github).getByRole("list", { name: "Contribuições por dia nos últimos 14 dias" })).toBeInTheDocument();
  });

  it("opens Settings from the module and returns focus to it", async () => {
    const { core } = fakeCore({ snapshots: [{ module: "github", updatedAt: 1, data: githubFull }] });
    render(Panel, { core });
    const github = await screen.findByRole("region", { name: "GitHub" });
    await userEvent.click(within(github).getByRole("button", { name: "Configurar GitHub" }));
    expect(screen.getByRole("heading", { name: "Configurações" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Voltar ao Painel" }));
    expect(within(github).getByRole("button", { name: "Configurar GitHub" })).toHaveFocus();
  });

  it("keeps focus and scroll position when a GitHub snapshot arrives", async () => {
    const { core, publish } = fakeCore({ snapshots: [{ module: "github", updatedAt: 1, data: githubFull }] });
    render(Panel, { core });
    const github = await screen.findByRole("region", { name: "GitHub" });
    const configure = within(github).getByRole("button", { name: "Configurar GitHub" });
    configure.focus();
    const scroller = screen.getByRole("main");
    scroller.scrollTop = 120;

    await publish({ module: "github", updatedAt: 2, data: { ...githubFull, refreshing: true } });

    expect(within(github).getByRole("status")).toHaveTextContent("Atualizando…");
    expect(configure).toHaveFocus();
    expect(scroller.scrollTop).toBe(120);
  });
});

describe("Trello module in the panel", () => {
  it("explains there is no board yet and sends a refresh to the core", async () => {
    const { core } = fakeCore();
    const refreshTrello = vi.fn();
    render(Panel, { core: { ...core, refreshTrello } });
    const trello = await screen.findByRole("region", { name: "Tarefas" });
    expect(within(trello).getByText("Sem leitura do quadro do Trello ainda")).toBeInTheDocument();
    await userEvent.click(within(trello).getByRole("button", { name: "Atualizar Tarefas" }));
    expect(refreshTrello).toHaveBeenCalledOnce();
  });

  it("moves and opens cards through the core from the board overlay, keeping the summary in step", async () => {
    const { core } = fakeCore({ snapshots: [{ module: "tasks", updatedAt: 1, data: trelloFull }] });
    const moveTrelloCard = vi.fn(async () => {});
    const openTrelloCard = vi.fn();
    render(Panel, { core: { ...core, moveTrelloCard, openTrelloCard } });
    const trello = await screen.findByRole("region", { name: "Tarefas" });
    const [from, to] = trelloFull.lists;
    const card = from.cards[0];
    await userEvent.click(within(trello).getByRole("button", { name: "Abrir quadro completo" }));
    const board = screen.getByRole("dialog", { name: "Quadro Trello" });
    await userEvent.click(within(board).getByRole("button", { name: new RegExp(`^${card.name}`) }));
    await userEvent.click(within(board).getByRole("button", { name: `Abrir no Trello: ${card.name}` }));
    expect(openTrelloCard).toHaveBeenCalledWith(card.url);
    await userEvent.click(within(within(board).getByRole("region", { name: to.name })).getByRole("button", { name: "Mover para esta lista" }));
    expect(moveTrelloCard).toHaveBeenCalledWith(card.id, to.id);
    const counts = within(trello).getByRole("list", { name: "Cards por lista" });
    expect(within(counts).getByText(to.name).nextElementSibling).toHaveTextContent("1");
  });

  it("reverts a move when the core cannot reach Trello", async () => {
    const { core } = fakeCore({ snapshots: [{ module: "tasks", updatedAt: 1, data: trelloFull }] });
    render(Panel, { core });
    const trello = await screen.findByRole("region", { name: "Tarefas" });
    const [from, to] = trelloFull.lists;
    await userEvent.click(within(trello).getByRole("button", { name: "Abrir quadro completo" }));
    const board = screen.getByRole("dialog", { name: "Quadro Trello" });
    await userEvent.click(within(board).getByRole("button", { name: new RegExp(`^${from.cards[0].name}`) }));
    await userEvent.click(within(within(board).getByRole("region", { name: to.name })).getByRole("button", { name: "Mover para esta lista" }));
    expect(await within(board).findByRole("alert")).toHaveTextContent("Não foi possível mover");
    expect(within(within(board).getByRole("region", { name: from.name })).getByText(from.cards[0].name)).toBeInTheDocument();
  });

  it("closes the board with Escape before Escape hides the panel, returning focus to its trigger", async () => {
    const { core } = fakeCore({ snapshots: [{ module: "tasks", updatedAt: 1, data: trelloFull }] });
    render(Panel, { core });
    const trello = await screen.findByRole("region", { name: "Tarefas" });
    const trigger = within(trello).getByRole("button", { name: "Abrir quadro completo" });
    await userEvent.click(trigger);
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(core.closePanel).not.toHaveBeenCalled();
    expect(trigger).toHaveFocus();
    await userEvent.keyboard("{Escape}");
    expect(core.closePanel).toHaveBeenCalledOnce();
  });

  it("opens Settings from the module and returns focus to it", async () => {
    const { core } = fakeCore({ snapshots: [{ module: "tasks", updatedAt: 1, data: trelloFull }] });
    render(Panel, { core });
    const trello = await screen.findByRole("region", { name: "Tarefas" });
    await userEvent.click(within(trello).getByRole("button", { name: "Configurar Tarefas" }));
    expect(screen.getByRole("heading", { name: "Configurações" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Voltar ao Painel" }));
    expect(within(trello).getByRole("button", { name: "Configurar Tarefas" })).toHaveFocus();
  });

  it("keeps the selected card, focus and scroll position when a Trello snapshot arrives", async () => {
    const { core, publish } = fakeCore({ snapshots: [{ module: "tasks", updatedAt: 1, data: trelloFull }] });
    render(Panel, { core });
    const trello = await screen.findByRole("region", { name: "Tarefas" });
    const scroller = screen.getByRole("main");
    scroller.scrollTop = 120;
    await userEvent.click(within(trello).getByRole("button", { name: "Abrir quadro completo" }));
    const board = screen.getByRole("dialog", { name: "Quadro Trello" });
    const card = within(board).getByRole("button", { name: new RegExp(`^${trelloFull.lists[0].cards[0].name}`) });
    await userEvent.click(card);

    await publish({ module: "tasks", updatedAt: 2, data: { ...trelloFull, refreshing: true } });

    expect(within(trello).getByText("Atualizando…")).toBeInTheDocument();
    expect(board).toHaveAttribute("open");
    expect(card).toHaveFocus();
    expect(card).toHaveAttribute("aria-pressed", "true");
    expect(scroller.scrollTop).toBe(120);
  });
});

describe("Network module in the panel", () => {
  it("explains there is no reading before the first network snapshot", async () => {
    const { core } = fakeCore();
    render(Panel, { core });
    const network = await screen.findByRole("region", { name: "Rede" });
    expect(within(network).getByText("Sem leitura da rede ainda")).toBeInTheDocument();
  });

  it("shows the instruments from the snapshot and sends a speed test to the core", async () => {
    const { core } = fakeCore({ snapshots: [networkFull as Snapshot] });
    const runSpeedTest = vi.fn();
    render(Panel, { core: { ...core, runSpeedTest } });
    const network = await screen.findByRole("region", { name: "Rede" });
    expect(within(network).getByRole("group", { name: "Internet" })).toHaveTextContent("20ms");
    expect(within(network).queryAllByRole("listitem")).toHaveLength(0);
    await userEvent.click(within(network).getByRole("button", { name: "5 aplicativos + Outros" }));
    expect(within(screen.getByRole("dialog", { name: "Aplicativos de rede" })).getAllByRole("listitem")).toHaveLength(6);
    await userEvent.keyboard("{Escape}");
    await userEvent.click(within(network).getByRole("button", { name: "Testar velocidade" }));
    expect(runSpeedTest).toHaveBeenCalledOnce();
  });

  it("keeps focus on the speed test when a network snapshot arrives", async () => {
    const { core, publish } = fakeCore({ snapshots: [networkFull as Snapshot] });
    render(Panel, { core });
    const network = await screen.findByRole("region", { name: "Rede" });
    within(network).getByRole("button", { name: "Testar velocidade" }).focus();
    await publish({ ...(networkFull as Snapshot), updatedAt: networkFull.updatedAt + 1000 });
    expect(within(network).getByRole("button", { name: "Testar velocidade" })).toHaveFocus();
  });
});

describe("System module in the panel", () => {
  it("explains there is no reading before the first system snapshot", async () => {
    const { core } = fakeCore();
    render(Panel, { core });
    const system = await screen.findByRole("region", { name: "Sistema" });
    expect(within(system).getByText("Sem leitura do sistema ainda")).toBeInTheDocument();
  });

  it("shows the instruments from the latest system snapshot", async () => {
    const { core, publish } = fakeCore({ snapshots: [notebook as Snapshot] });
    render(Panel, { core });
    const system = await screen.findByRole("region", { name: "Sistema" });
    expect(await within(system).findByRole("group", { name: "CPU" })).toHaveTextContent("2%");
    expect(within(system).getByRole("meter", { name: "RAM" })).toBeInTheDocument();

    const data = structuredClone(notebook.data);
    data.cpu.usage = 37.6;
    await publish({ ...(notebook as Snapshot), updatedAt: notebook.updatedAt + 2000, data });
    expect(within(system).getByRole("group", { name: "CPU" })).toHaveTextContent("38%");
  });
});


describe("Decorative choreography", () => {
  it("stops all decoration in Settings and resumes on keyboard return with readings intact", async () => {
    const { core } = fakeCore({ snapshots: [{ module: "ai", updatedAt: 1, data: aiFull }] });
    render(Panel, { core });
    const meter = await screen.findByRole("meter", { name: "Codex · janela de 5 h" });
    const reading = meter.outerHTML;
    await vi.waitFor(() => expect(document.documentElement.dataset.decoration).toBe("running"));
    const settings = screen.getByRole("button", { name: "Configurações" });
    settings.focus();
    await userEvent.keyboard("{Enter}");
    expect(document.documentElement.dataset.decoration).toBe("paused");
    screen.getByRole("button", { name: "Voltar ao Painel" }).focus();
    await userEvent.keyboard("{Enter}");
    expect(document.documentElement.dataset.decoration).toBe("running");
    expect(settings).toHaveFocus();
    expect(meter.outerHTML).toBe(reading);
  });

  it("waits for native visibility, stops while hidden, and preserves instrument readings", async () => {
    const { core } = fakeCore({ snapshots: [{ module: "ai", updatedAt: 1, data: aiFull }] });
    let visibility: ((visible: boolean) => void) | undefined;
    const stop = vi.fn();
    const view = render(Panel, { core: { ...core,
      initialState: async () => ({ lastAnchor: "ai", snapshots: [{ module: "ai", updatedAt: 1, data: aiFull }], panelVisible: false }),
      listenPanelVisibility: async (cb: (visible: boolean) => void) => { visibility = cb; return stop; },
    } });
    const meter = await screen.findByRole("meter", { name: "Codex · janela de 5 h" });
    const reading = meter.outerHTML;
    expect(document.documentElement.dataset.decoration).toBe("paused");
    await act(() => visibility?.(true));
    expect(document.documentElement.dataset.decoration).toBe("running");
    await act(() => visibility?.(false));
    expect(document.documentElement.dataset.decoration).toBe("paused");
    expect(meter.outerHTML).toBe(reading);
    await act(() => visibility?.(true));
    expect(document.documentElement.dataset.decoration).toBe("running");
    view.unmount();
    await Promise.resolve();
    expect(stop).toHaveBeenCalledOnce();
    expect(document.documentElement.dataset.decoration).toBe("paused");
  });
});


it("restores choreography only after a recreated webview receives native state", async () => {
  const { core } = fakeCore();
  const visibleCore = { ...core, initialState: async (): Promise<InitialState> => ({ lastAnchor: "ai", snapshots: [], panelVisible: true }) };
  const first = render(Panel, { core: visibleCore });
  await vi.waitFor(() => expect(document.documentElement.dataset.decoration).toBe("running"));
  first.unmount();
  expect(document.documentElement.dataset.decoration).toBe("paused");
  render(Panel, { core: visibleCore });
  await vi.waitFor(() => expect(document.documentElement.dataset.decoration).toBe("running"));
});

it("recovers one source without replacing another source's reading or selected card", async () => {
  const invalidGithub = { ...githubFull, state: "invalidCredential" };
  const { core, publish } = fakeCore({ snapshots: [
    { module: "ai", updatedAt: 1, data: aiFull },
    { module: "github", updatedAt: 1, data: invalidGithub },
    { module: "tasks", updatedAt: 1, data: trelloFull },
  ] });
  render(Panel, { core });
  const meter = await screen.findByRole("meter", { name: "Codex · janela de 5 h" });
  const value = meter.getAttribute("aria-valuenow");
  const scroller = screen.getByRole("main");
  scroller.scrollTop = 180;
  await userEvent.click(within(section("Tarefas")).getByRole("button", { name: "Abrir quadro completo" }));
  const card = within(screen.getByRole("dialog", { name: "Quadro Trello" }))
    .getByRole("button", { name: new RegExp(`^${trelloFull.lists[0].cards[0].name}`) });
  await userEvent.click(card);
  expect(within(section("GitHub")).getByRole("button", { name: "Reconectar" })).toBeInTheDocument();

  await publish({ module: "github", updatedAt: 2, data: githubFull });

  expect(within(section("GitHub")).queryByRole("button", { name: "Reconectar" })).not.toBeInTheDocument();
  expect(meter).toHaveAttribute("aria-valuenow", value);
  expect(card).toHaveAttribute("aria-pressed", "true");
  expect(card).toHaveFocus();
  expect(scroller.scrollTop).toBe(180);
});


it("dismisses an AI description before Escape closes the panel", async () => {
  const { core } = fakeCore();
  render(Panel, { core });
  const configure = await screen.findByRole("button", { name: "Configurar IA" });
  configure.focus();
  await userEvent.keyboard("{Escape}");
  expect(core.closePanel).not.toHaveBeenCalled();
  expect(configure).toHaveFocus();
  await userEvent.keyboard("{Escape}");
  expect(core.closePanel).toHaveBeenCalledOnce();
});

it("dismisses a GitHub description before Escape closes the panel", async () => {
  const { core } = fakeCore();
  render(Panel, { core });
  const configure = await screen.findByRole("button", { name: "Configurar GitHub" });
  configure.focus();
  await userEvent.keyboard("{Escape}");
  expect(core.closePanel).not.toHaveBeenCalled();
  expect(configure).toHaveFocus();
  await userEvent.keyboard("{Escape}");
  expect(core.closePanel).toHaveBeenCalledOnce();
});

it("preserves notification selection when reopening from the panel ring", async () => {
  const { core } = fakeCore({ agentTarget: waitingAgents.sessions[0].id,
    snapshots: [{ module: "agents", updatedAt: 1000, data: waitingAgents }] });
  render(Panel, { core });
  await screen.findByRole("listitem", { current: true });
  await userEvent.keyboard("{Escape}");
  expect(core.chooseAnchor).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Sessões de agente" }));
  expect(screen.getByRole("listitem", { current: true })).toHaveTextContent("Minha sessão");
  await userEvent.keyboard("{Escape}");
  expect(core.closePanel).not.toHaveBeenCalled();
  await userEvent.keyboard("{Escape}");
  expect(core.closePanel).toHaveBeenCalledOnce();
});

it("closes the network apps detail before Escape hides the panel, keeping the anchor", async () => {
  const { core } = fakeCore({ lastAnchor: "github", snapshots: [{ module: "network", updatedAt: 1000, data: networkFull.data }] });
  render(Panel, { core });
  const trigger = await screen.findByRole("button", { name: "5 aplicativos + Outros" });
  await userEvent.click(trigger);
  const detail = screen.getByRole("dialog", { name: "Aplicativos de rede" });
  expect(within(detail).getByText("firefox")).toBeInTheDocument();
  vi.mocked(core.chooseAnchor).mockClear();
  await userEvent.keyboard("{Escape}");
  expect(detail).not.toHaveAttribute("open");
  expect(trigger).toHaveFocus();
  expect(core.closePanel).not.toHaveBeenCalled();
  expect(core.chooseAnchor).not.toHaveBeenCalled();
  await userEvent.keyboard("{Escape}");
  expect(core.closePanel).toHaveBeenCalledOnce();
});

it("opening the network apps detail does not change what the core collects", async () => {
  const { core } = fakeCore({ snapshots: [{ module: "network", updatedAt: 1000, data: networkFull.data }] });
  const runSpeedTest = vi.fn();
  render(Panel, { core: { ...core, runSpeedTest } });
  const trigger = await screen.findByRole("button", { name: "5 aplicativos + Outros" });
  const reported = vi.mocked(core.reportVisibleModules).mock.calls.length;
  await userEvent.click(trigger);
  await userEvent.keyboard("{Escape}");
  expect(vi.mocked(core.reportVisibleModules).mock.calls.length).toBe(reported);
  expect(runSpeedTest).not.toHaveBeenCalled();
});

it("keeps GitHub failure details and keyboard focus after the strip result expires", async () => {
  vi.useFakeTimers();
  try {
    const { core, publish } = fakeCore({ snapshots: [{ module: "github", updatedAt: 1, data: githubFull }] });
    render(Panel, { core });
    await act(() => Promise.resolve());
    const github = section("GitHub");
    github.focus();
    const { default: failure } = await import("./github/fixtures/failure.json");
    await publish({ module: "github", updatedAt: 1, data: { ...failure, operation: { id: 2, origin: "user", state: "failed" } } });
    expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("GitHub · Falha ao atualizar");
    expect(github).toHaveFocus();
    await act(() => vi.advanceTimersByTime(4000));
    expect(screen.getByLabelText("Faixa de bordo")).toHaveTextContent("Bordo em repouso");
    expect(within(github).getByRole("button", { name: "Atualizar GitHub" })).toBeEnabled();
    expect(github).toHaveTextContent("Falha ao atualizar");
    expect(github).toHaveFocus();
  } finally {
    vi.useRealTimers();
  }
});

it("keeps six modules accessible while simultaneous results expire independently of a long query", async () => {
  vi.useFakeTimers();
  try {
    const { default: githubFailure } = await import("./github/fixtures/failure.json");
    const { default: trelloOffline } = await import("./trello/fixtures/offline.json");
    const { default: systemWarning } = await import("./system/fixtures/warning.json");
    const { core, publishAll } = fakeCore({ snapshots: [
      { module: "ai", updatedAt: 1793460871000, data: aiFull },
      { module: "agents", updatedAt: 1793460871000, data: { ...waitingAgents, sessions: [], closed: [] } },
      { module: "github", updatedAt: 1793460871000, data: githubFull },
      { module: "tasks", updatedAt: 1793460871000, data: trelloFull },
      { module: "network", updatedAt: 1793460871000, data: networkFull.data },
      { module: "system", updatedAt: 1793460871000, data: notebook.data },
    ] });
    render(Panel, { core });
    await act(() => Promise.resolve());
    const github = section("GitHub");
    github.focus();
    await publishAll([
      { module: "ai", updatedAt: 1793460871000, data: { ...aiFull, codex: { ...aiFull.codex, operation: { id: 100, origin: "user", state: "running" } } } },
      { module: "agents", updatedAt: 1793460871000, data: waitingAgents },
      { module: "github", updatedAt: 1793460871000, data: githubFailure },
      { module: "tasks", updatedAt: 1793460871000, data: trelloOffline },
      { module: "network", updatedAt: 1793460871000, data: { ...networkFull.data, speedOperation: { id: 100, origin: "user", state: "cancelled" } } },
      { module: "system", updatedAt: 1793460871000, data: systemWarning.data },
    ]);
    await act(() => Promise.resolve());
    const strip = screen.getByLabelText("Faixa de bordo");
    expect(strip).toHaveTextContent("GitHub · Falha ao atualizar");
    expect(strip).toHaveTextContent("+3 Módulos");
    const announcement = screen.getAllByRole("status").find(element => element.getAttribute("aria-live") === "polite");
    expect(announcement).toHaveAttribute("aria-atomic", "true");
    expect(announcement).toHaveTextContent("GitHub");
    expect(announcement).toHaveTextContent("Sistema");
    expect(github).toHaveFocus();
    await act(() => vi.advanceTimersByTime(3999));
    expect(strip).toHaveTextContent("GitHub · Falha ao atualizar");
    await act(() => vi.advanceTimersByTime(1));
    expect(strip).toHaveTextContent("Codex");
    expect(strip).not.toHaveTextContent("GitHub");
    expect(github).toHaveTextContent("Falha ao atualizar");
    expect(within(github).getByRole("button", { name: "Atualizar GitHub" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Abrir quadro completo" })).toBeEnabled();
    expect(github).toHaveFocus();
    await act(() => vi.advanceTimersByTime(60_000));
    expect(strip).toHaveTextContent("Codex");
    expect(strip).not.toHaveTextContent("Telemetria atualizada");
  } finally {
    vi.useRealTimers();
  }
});

// Synthetic native capability snapshot: unavailable metrics must not look healthy.
it("explains unavailable native modules instead of rendering metric values or actions", async () => {
  const { core } = fakeCore({ unavailableModules: ["ai", "network", "system"], snapshots: [{ module: "system", updatedAt: 1000, data: notebook }] });
  render(Panel, { core });
  expect(await screen.findAllByText("Indisponível no Windows nesta versão de validação.")).toHaveLength(3);
  expect(within(section("Sistema")).queryByText(/CPU/)).not.toBeInTheDocument();
  expect(within(section("Rede")).queryByRole("button")).not.toBeInTheDocument();
});

it("does not offer credential writes for an unavailable Windows module", async () => {
  const { core } = fakeCore({ unavailableModules: ["ai"] });
  const connectClaude = vi.fn();
  render(Panel, { core: { ...core, connectClaude } });
  await screen.findByRole("region", { name: "IA" });
  await userEvent.click(settingsButton());
  await userEvent.click(screen.getByRole("button", { name: /^IA$/ }));
  expect(document.querySelector(".pane.ai")).toHaveTextContent("Indisponível no Windows nesta versão de validação.");
  expect(connectClaude).not.toHaveBeenCalled();
});

it("opens a module-only notification at Agents even when first-use welcome is pending", async () => {
  const { core } = fakeCore({ lastAnchor: "github", agentTarget: "" });
  const completeWelcome = vi.fn();
  const prepared = vi.fn();
  const initialState = vi.fn(async () => ({ lastAnchor: "github" as const, agentTarget: "", welcome: true, panelVisible: true, snapshots: [] }));
  render(Panel, { core: { ...core, initialState, completeWelcome }, prepared });
  await vi.waitFor(() => expect(prepared).toHaveBeenCalled());
  expect(vi.mocked(Element.prototype.scrollIntoView).mock.contexts).toContain(section("Agentes"));
  expect(settingsButton()).not.toHaveAttribute("aria-current", "page");
  expect(completeWelcome).not.toHaveBeenCalled();
  expect(core.chooseAnchor).not.toHaveBeenCalled();
});


it("accepts a newer revision clearing the previous context even with a null reading time", async () => {
  const initial = { module: "tasks" as const, updatedAt: 10, data: trelloFull, revision: 1, runtimeId: "test-runtime", contextGeneration: 1 };
  const { core, publish } = fakeCore({ runtimeId: "test-runtime", snapshots: [initial] });
  render(Panel, { core });
  expect(await screen.findByText("17 cards · 8 listas")).toBeInTheDocument();
  const cleared = { module: "tasks" as const, updatedAt: null, revision: 2, runtimeId: "test-runtime", contextGeneration: 2,
    data: { state: "missingCredential", refreshing: false, account: null, boards: [], board: null, lists: null } };
  await publish(cleared);
  expect(screen.queryByText("17 cards · 8 listas")).not.toBeInTheDocument();
  expect(within(section("Tarefas")).getByRole("button", { name: "Configurar Tarefas" })).toBeInTheDocument();
});

it("orders readings by revision when wall time moves back and ignores other runtimes", async () => {
  const initial: Snapshot = { module: "tasks", updatedAt: 1000, data: trelloFull, revision: 3, runtimeId: "current" };
  const { core, publish } = fakeCore({ runtimeId: "current", snapshots: [initial] });
  render(Panel, { core });
  await screen.findByText("17 cards · 8 listas");
  const changed = { ...trelloFull, lists: [] };
  await publish({ ...initial, updatedAt: 1, revision: 4, data: changed });
  expect(screen.queryByText("17 cards · 8 listas")).not.toBeInTheDocument();
  await publish({ ...initial, updatedAt: 9999, revision: 3 });
  await publish({ ...initial, runtimeId: "previous", revision: 999 });
  expect(screen.queryByText("17 cards · 8 listas")).not.toBeInTheDocument();
  await publish({ ...initial, updatedAt: 2, revision: 5 });
  expect(screen.getByText("17 cards · 8 listas")).toBeInTheDocument();
});

it("keeps events received during bootstrap ahead of the initial snapshot", async () => {
  const initial: Snapshot = { module: "tasks", updatedAt: 1000, data: trelloFull, revision: 1, runtimeId: "current" };
  const { core, publish } = fakeCore();
  let finish!: (state: InitialState) => void;
  core.initialState.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  render(Panel, { core });
  await publish({ ...initial, updatedAt: null, revision: 2, contextGeneration: 2,
    data: { state: "cleanupPending", provider: "trello", contextGeneration: 2, canWrite: false, boards: [], lists: null } });
  await publish({ ...initial, runtimeId: "previous", revision: 999 });
  await act(() => finish({ runtimeId: "current", lastAnchor: "tasks", snapshots: [initial], panelVisible: true, agentTarget: null }));
  expect(screen.queryByText("17 cards · 8 listas")).not.toBeInTheDocument();
  expect(within(section("Tarefas")).getByText(/limpeza/i)).toBeInTheDocument();
});
