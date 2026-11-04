import { act, render as renderComponent, screen, within } from "@testing-library/svelte";
import { afterEach, expect, it, vi } from "vitest";
import PanelOpening, { type OpeningClock } from "./PanelOpening.svelte";
import type { Core, InitialState } from "./core";
import github from "./github/fixtures/full.json";

const clock: OpeningClock = {
  presented(callback) { const timer = setTimeout(callback, 0); return () => clearTimeout(timer); },
  after(ms, callback) { const timer = setTimeout(callback, ms); return () => clearTimeout(timer); },
};
const render = (_: typeof PanelOpening, props: { core: Core }) => renderComponent(PanelOpening, { ...props, clock });
const initial: InitialState = { lastAnchor: "github", panelVisible: true, snapshots: [{ module: "github", updatedAt: 1, data: github }] };
function core(load: () => Promise<InitialState>): Core {
  return { initialState: load, listenSnapshots: async () => () => {}, reportVisibleModules: vi.fn(), chooseAnchor: vi.fn(), closePanel: vi.fn() };
}
afterEach(() => vi.useRealTimers());
it("presents local fixtures only after the visible minimum, without inventing work", async () => {
  vi.useFakeTimers();
  render(PanelOpening, { core: core(async () => initial) });
  await act(() => vi.advanceTimersByTimeAsync(0));
  expect(screen.queryByRole("region", { name: "GitHub" })).not.toBeInTheDocument();
  expect(screen.getByRole("status")).toBeEmptyDOMElement();
  expect(screen.getByRole("heading", { name: "kvojps monitor" })).toBeVisible();
  await act(() => vi.advanceTimersByTimeAsync(1199));
  expect(screen.queryByRole("region", { name: "GitHub" })).not.toBeInTheDocument();
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(screen.getAllByRole("region", { name: "GitHub" })[0]).toBeVisible();
});

it("reveals slow local preparation with only the transition after readiness", async () => {
  vi.useFakeTimers();
  let resolve!: (state: InitialState) => void;
  render(PanelOpening, { core: core(() => new Promise((done) => { resolve = done; })) });
  await act(() => vi.advanceTimersByTimeAsync(3000));
  expect(screen.getByRole("status")).toHaveTextContent("Preparando o Painel");
  await act(async () => { resolve(initial); });
  await act(() => vi.advanceTimersByTimeAsync(0));
  expect(screen.queryByRole("region", { name: "GitHub" })).not.toBeInTheDocument();
  await act(() => vi.advanceTimersByTimeAsync(240));
  expect(screen.getAllByRole("region", { name: "GitHub" })[0]).toBeVisible();
});

it.each(["timeout", "rejection"])("recovers from %s with defaults and ignores late local data", async (failure) => {
  vi.useFakeTimers();
  let resolve!: (state: InitialState) => void;
  const source = core(failure === "timeout" ? () => new Promise((done) => { resolve = done; }) : async () => { throw new Error("sensitive source detail"); });
  render(PanelOpening, { core: source });
  await act(() => vi.advanceTimersByTimeAsync(5240));
  expect(screen.getAllByRole("region", { name: "IA" })[0]).toBeVisible();
  expect(screen.getByText(/Preferências não restauradas/)).toHaveTextContent("valores padrão");
  expect(document.body).not.toHaveTextContent("sensitive source detail");
  if (failure === "timeout") {
    await act(async () => resolve(initial));
    expect(screen.getByText(/Preferências não restauradas/)).toHaveTextContent("valores padrão");
    expect(screen.getAllByRole("region", { name: "GitHub" })[0]).not.toHaveTextContent("Commits");
  }
});

it("cancels a closed opening and starts a new sequence on remount", async () => {
  vi.useFakeTimers();
  let resolve!: (state: InitialState) => void;
  const source = core(() => new Promise((done) => { resolve = done; }));
  const first = render(PanelOpening, { core: source });
  await act(() => vi.advanceTimersByTimeAsync(0));
  first.unmount();
  const second = render(PanelOpening, { core: core(async () => initial) });
  await act(async () => resolve({ lastAnchor: "tasks", snapshots: [] }));
  await act(() => vi.advanceTimersByTimeAsync(1199));
  expect(screen.queryByRole("region", { name: "GitHub" })).not.toBeInTheDocument();
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(screen.getAllByRole("region", { name: "GitHub" })[0]).toBeVisible();
  second.unmount();
  expect(vi.getTimerCount()).toBe(0);
});

it("offers retry and close after a fatal interface failure", async () => {
  vi.useFakeTimers();
  let broken = true;
  const source = core(async () => initial);
  source.listenSnapshots = () => { if (broken) throw new Error("private failure"); return Promise.resolve(() => {}); };
  render(PanelOpening, { core: source });
  await act(() => vi.advanceTimersByTimeAsync(0));
  expect(screen.getByRole("alert")).toHaveTextContent("Não foi possível abrir o Painel");
  expect(document.body).not.toHaveTextContent("private failure");
  expect(screen.getByRole("button", { name: "Fechar" })).toBeEnabled();
  broken = false;
  await act(() => screen.getByRole("button", { name: "Tentar novamente" }).click());
  await act(() => vi.advanceTimersByTimeAsync(1200));
  expect(screen.getAllByRole("region", { name: "GitHub" })[0]).toBeVisible();
});

it("keeps pending remote work outside readiness and updates modules without replacing the panel", async () => {
  vi.useFakeTimers();
  let publish!: Parameters<Core["listenSnapshots"]>[0];
  const source = core(async () => ({ lastAnchor: "ai", panelVisible: true, snapshots: [] }));
  source.listenSnapshots = (listener) => { publish = listener; return new Promise(() => {}); };
  render(PanelOpening, { core: source });
  await act(() => vi.advanceTimersByTimeAsync(1200));
  const module = screen.getAllByRole("region", { name: "GitHub" })[0];
  expect(module).toBeVisible();
  module.focus();
  await act(() => publish(initial.snapshots[0]));
  expect(screen.getAllByRole("region", { name: "GitHub" })[0]).toBe(module);
  expect(module).toHaveFocus();
});

it("closes once with Escape during preparation and releases timers", async () => {
  vi.useFakeTimers();
  const source = core(() => new Promise(() => {}));
  render(PanelOpening, { core: source });
  await act(() => vi.advanceTimersByTimeAsync(0));
  await act(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", cancelable: true })));
  expect(source.closePanel).toHaveBeenCalledTimes(1);
  expect(vi.getTimerCount()).toBe(0);
});

it("restores welcome after the sequence without issuing credential or hook commands", async () => {
  vi.useFakeTimers();
  const source = { ...core(async () => ({ ...initial, welcome: true })), completeWelcome: vi.fn(), connectClaude: vi.fn(), setAgentHooks: vi.fn(), saveGithubToken: vi.fn() };
  render(PanelOpening, { core: source });
  await act(() => vi.advanceTimersByTimeAsync(1200));
  expect(screen.getByRole("button", { name: "Voltar ao Painel" })).toBeVisible();
  expect(source.completeWelcome).toHaveBeenCalledOnce();
  expect(source.connectClaude).not.toHaveBeenCalled();
  expect(source.setAgentHooks).not.toHaveBeenCalled();
  expect(source.saveGithubToken).not.toHaveBeenCalled();
});

it("recovers missing areas while retaining live local fixtures", async () => {
  vi.useFakeTimers();
  let publish!: Parameters<Core["listenSnapshots"]>[0];
  const source = core(() => new Promise(() => {}));
  source.listenSnapshots = async (listener) => { publish = listener; return () => {}; };
  render(PanelOpening, { core: source });
  await act(() => vi.advanceTimersByTimeAsync(100));
  await act(() => publish(initial.snapshots[0]));
  await act(() => vi.advanceTimersByTimeAsync(5140));
  const githubModule = screen.getAllByRole("region", { name: "GitHub" })[0];
  expect(githubModule).toBeVisible();
  expect(within(githubModule).queryByText(/Restauração local indisponível/)).not.toBeInTheDocument();
  expect(within(screen.getAllByRole("region", { name: "IA" })[0]).getByText(/Restauração local indisponível/)).toBeVisible();
});

it("counts the minimum from presentation rather than mounting", async () => {
  vi.useFakeTimers();
  let present!: () => void;
  renderComponent(PanelOpening, { core: core(async () => initial), clock: { ...clock, presented(callback) { present = callback; return () => {}; } } });
  await act(() => vi.advanceTimersByTimeAsync(2000));
  expect(screen.queryByRole("region", { name: "GitHub" })).not.toBeInTheDocument();
  await act(() => present());
  await act(() => vi.advanceTimersByTimeAsync(1199));
  expect(screen.queryByRole("region", { name: "GitHub" })).not.toBeInTheDocument();
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(screen.getAllByRole("region", { name: "GitHub" })[0]).toBeVisible();
});


it.each([
  ["light", false, false, "light", "full"],
  ["dark", true, false, "dark", "reduced"],
  ["system", false, true, "light", "reduced"],
] as const)("restores %s appearance behind the opening (manual=%s, system=%s)", async (theme, reduceMotion, systemReduced, effectiveTheme, motion) => {
  vi.useFakeTimers();
  vi.stubGlobal("matchMedia", (query: string) => ({ matches: query.includes("reduced-motion") ? systemReduced : true, addEventListener() {}, removeEventListener() {} }));
  try {
    const source = core(async () => ({ ...initial, appearance: { theme, reduceMotion } }));
    const view = render(PanelOpening, { core: source });
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(document.documentElement.dataset.theme).toBe(effectiveTheme);
    expect(document.documentElement.dataset.motion).toBe(motion);
    expect(document.documentElement.dataset.decoration).toBe("paused");
    expect(screen.getByRole("heading", { name: "kvojps monitor" })).toBeVisible();
    expect(view.container.querySelector("[aria-hidden='true']")).toHaveProperty("inert", true);
    await act(() => vi.advanceTimersByTimeAsync(960));
    expect(screen.queryByRole("region", { name: "GitHub" })).not.toBeInTheDocument();
    expect(document.documentElement.dataset.decoration).toBe("paused");
    await act(() => vi.advanceTimersByTimeAsync(240));
    expect(screen.queryByRole("heading", { name: "kvojps monitor" })).not.toBeInTheDocument();
    expect(screen.getAllByRole("region", { name: "GitHub" })[0]).toBeVisible();
    expect(document.documentElement.dataset.decoration).toBe(motion === "reduced" ? "paused" : "running");
    view.unmount();
    expect(document.documentElement.dataset.decoration).toBe("paused");
  } finally { vi.unstubAllGlobals(); }
});


it("replaces the initial HTML composition without leaving duplicate identity", async () => {
  vi.useFakeTimers();
  const bootstrap = document.createElement("div");
  bootstrap.id = "opening-bootstrap";
  bootstrap.innerHTML = "<h1>kvojps monitor</h1>";
  document.body.append(bootstrap);
  render(PanelOpening, { core: core(async () => initial) });
  await act(() => vi.advanceTimersByTimeAsync(0));
  expect(bootstrap).not.toBeInTheDocument();
  expect(screen.getAllByRole("heading", { name: "kvojps monitor" })).toHaveLength(1);
  await act(() => vi.advanceTimersByTimeAsync(1200));
  expect(screen.queryByRole("heading", { name: "kvojps monitor" })).not.toBeInTheDocument();
  expect(screen.getAllByRole("region", { name: "GitHub" })[0]).toBeVisible();
});
