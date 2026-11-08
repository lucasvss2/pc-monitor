import { render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import SystemModule from "./SystemModule.svelte";
import type { SystemData, Snapshot } from "../core";
// Snapshots published by the core; regenerate with UPDATE_FIXTURES=1 cargo test.
import notebook from "./fixtures/notebook.json";
import desktop from "./fixtures/desktop.json";
import noSensor from "./fixtures/no-sensor.json";
import missing from "./fixtures/missing.json";
import warning from "./fixtures/warning.json";
import starting from "./fixtures/starting.json";

function renderSnapshot(snapshot: Snapshot) {
  render(SystemModule, { data: snapshot.data as SystemData, updatedAt: snapshot.updatedAt });
}

const group = (name: string) => screen.getByRole("group", { name });
const meter = (name: string) => screen.getByRole("meter", { name });

afterEach(() => vi.useRealTimers());

/** Render a snapshot as seen some time after the core published it. */
function renderLater(snapshot: Snapshot, elapsed: number) {
  vi.useFakeTimers({ now: snapshot.updatedAt! + elapsed, toFake: ["Date", "setInterval", "clearInterval"] });
  renderSnapshot(snapshot);
}

describe("System module", () => {
  it("keeps each GPU's usage and temperature together and distinguishes unavailable readings", () => {
    const data = structuredClone(notebook.data) as SystemData;
    data.graphics = { failed: false, devices: [
      { label: "GPU 0 · AMD", usage: { state: "ready", value: 0 }, temperature: { state: "unavailable" } },
      { label: "GPU 1 · NVIDIA", usage: { state: "ready", value: 54.2 }, temperature: { state: "ready", value: 43.5 } },
    ] };
    render(SystemModule, { data, updatedAt: notebook.updatedAt });
    expect(group("GPU 0 · AMD · Uso")).toHaveTextContent("0%");
    expect(group("GPU 0 · AMD · Temperatura da GPU")).toHaveTextContent("Indisponível");
    expect(group("GPU 0 · AMD · Temperatura da GPU")).not.toHaveTextContent("°C");
    expect(group("GPU 1 · NVIDIA · Uso")).toHaveTextContent("54%");
    expect(group("GPU 1 · NVIDIA · Temperatura da GPU")).toHaveTextContent("43,5°C");
    expect(group("CPU")).toHaveTextContent("2%");
  });

  it("explains GPU warm-up and failure while leaving CPU and memory available", () => {
    const data = structuredClone(notebook.data) as SystemData;
    data.graphics = { failed: false, devices: [
      { label: "GPU 0 · NVIDIA", usage: { state: "measuring" }, temperature: { state: "failed" } },
    ] };
    render(SystemModule, { data, updatedAt: notebook.updatedAt });
    expect(group("GPU 0 · NVIDIA · Uso")).toHaveTextContent("Medindo…");
    expect(group("GPU 0 · NVIDIA · Uso")).not.toHaveTextContent("0%");
    expect(group("GPU 0 · NVIDIA · Temperatura da GPU")).toHaveTextContent("Falha na leitura");
    expect(meter("RAM")).toBeInTheDocument();
  });

  it("shows B's instruments from a notebook: CPU, temperature, RAM, swap, root disk and battery", () => {
    renderSnapshot(notebook as Snapshot);

    expect(screen.getByRole("heading", { level: 2, name: "Sistema" })).toBeInTheDocument();
    expect(group("CPU")).toHaveTextContent("2%");
    expect(group("Temperatura")).toHaveTextContent("47,4°C");
    expect(screen.getAllByRole("meter").map((m) => m.getAttribute("aria-label"))).toEqual(["RAM", "Swap", "Disco /"]);
    expect(meter("RAM")).toHaveAttribute("aria-valuenow", "39");
    expect(meter("RAM")).toHaveAttribute("aria-valuetext", "39% · 5,7 / 14,5 GB");
    expect(meter("Swap")).toHaveAttribute("aria-valuetext", "9% · 0,4 / 4,0 GB");
    // The percentage agrees with the pair; the core still decides the warning.
    expect(meter("Disco /")).toHaveAttribute("aria-valuetext", "48% · 70,4 / 146,6 GB");
    expect(screen.getByText("39%")).toBeInTheDocument();
    expect(screen.getByText("9%")).toBeInTheDocument();
    expect(screen.getByText("48%")).toBeInTheDocument();
    expect(screen.getByText("Bateria 100% · Carregada")).toHaveClass("battery");
  });

  it("shows GB used/total only under RAM, as B does", () => {
    renderSnapshot(notebook as Snapshot);

    expect(screen.getByText("5,7 / 14,5 GB")).toBeInTheDocument();
    expect(screen.queryByText(/0,4 \/ 4,0 GB|70,4 \/ 146,6 GB/)).not.toBeInTheDocument();
  });

  it("shows nothing beyond B in the normal state", () => {
    renderLater(notebook as Snapshot, 3_000);

    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.queryByRole("meter", { name: "Bateria" })).not.toBeInTheDocument();
    expect(screen.queryByText(/Kernel|UPower|atualizado|livres|⚠|Sem /)).not.toBeInTheDocument();
  });

  it("says a desktop has no battery instead of showing a charge", () => {
    renderSnapshot(desktop as Snapshot);

    expect(screen.getByText("Sem bateria detectada")).toHaveClass("battery", "missing");
    expect(screen.getByText("Sem bateria detectada")).toHaveTextContent("▱ Sem bateria detectada");
    expect(screen.queryByText(/^Bateria/)).not.toBeInTheDocument();
    expect(group("CPU")).toHaveTextContent("2%");
  });

  it("explains a missing temperature sensor without inventing a value", () => {
    renderSnapshot(noSensor as Snapshot);

    expect(group("Temperatura · sem sensor")).toHaveTextContent("—");
    expect(group("Temperatura · sem sensor")).not.toHaveTextContent("°C");
  });

  it("distinguishes Windows capabilities from missing hardware", () => {
    const data = { ...missing.data, disks: [], unavailableMetrics: ["temperature", "swap", "battery", "storage"] } as SystemData;
    render(SystemModule, { data, updatedAt: missing.updatedAt });

    expect(meter("RAM")).toBeInTheDocument();
    expect(group("Temperatura · coleta indisponível no Windows")).toHaveTextContent("—");
    expect(screen.queryByText(/swap/i)).not.toBeInTheDocument();
    expect(screen.getByText("Bateria · leitura indisponível")).toBeInTheDocument();
    expect(screen.getByText("Armazenamento · leitura indisponível")).toBeInTheDocument();
  });

  it("explains missing sensor, swap and battery together", () => {
    renderSnapshot(missing as Snapshot);

    // Each missing item keeps its place in B's composition, with the reason in it.
    expect(group("Temperatura · sem sensor")).toHaveTextContent("—");
    expect(screen.queryByRole("meter", { name: "Swap" })).not.toBeInTheDocument();
    const swap = screen.getByText("Sem swap configurada").closest(".meter")!;
    expect(swap).toHaveTextContent(/^Swap\s*—\s*Sem swap configurada$/);
    expect(swap.querySelector(".track")).toBeInTheDocument();
    expect(screen.getByText("Sem bateria detectada")).toHaveClass("battery");
    expect(screen.getByText("Sem swap configurada")).not.toHaveClass("line");
    expect(meter("RAM")).toBeInTheDocument();
  });

  it("identifies temperature, disk and battery alerts as body lines, not only color", () => {
    renderSnapshot(warning as Snapshot);

    expect(group("Temperatura")).toHaveTextContent("92,1°C");
    expect(screen.getByText("Temperatura alta")).toHaveClass("line", "alert");
    expect(screen.getByText("Bateria baixa")).toHaveClass("line", "alert");
    const battery = screen.getByText("Bateria 12% · Na bateria");
    expect(battery).toHaveClass("warning");
    expect(battery).not.toHaveTextContent(/restantes|min/);
    // A warning disk other than the root one keeps its alert without listing every disk.
    expect(screen.getAllByRole("meter").map((m) => m.getAttribute("aria-label"))).toEqual(["RAM", "Swap", "Disco /"]);
    expect(screen.getByRole("alert")).toHaveTextContent("Pouco espaço em /mnt/data");
  });

  it("flags the root disk itself when it is almost full", () => {
    const data = structuredClone(notebook.data) as SystemData;
    data.disks = [{ ...data.disks[0], free: 8_000_000_000, warning: true }];
    render(SystemModule, { data, updatedAt: notebook.updatedAt });

    expect(screen.getByRole("alert")).toHaveTextContent("Pouco espaço em /");
    expect(meter("Disco /")).toHaveClass("warning");
  });

  it("says CPU usage is being measured until two samples exist, never 0%", () => {
    renderSnapshot(starting as Snapshot);

    expect(group("CPU")).toHaveTextContent("Medindo…");
    expect(group("CPU")).not.toHaveTextContent("0%");
  });

  it("explains there is no reading before the first system snapshot", () => {
    render(SystemModule, { data: null, updatedAt: null });

    expect(screen.getByRole("heading", { level: 2, name: "Sistema" })).toBeInTheDocument();
    const area = screen.getByText("Sem leitura do sistema ainda").closest(".area");
    expect(area).toHaveAttribute("data-tone", "neutral");
    expect(area).toHaveTextContent("◌ Sem leitura do sistema ainda");
    expect(screen.queryByRole("meter")).not.toBeInTheDocument();
  });

  it("keeps the last reading with its age when collection stops arriving", () => {
    renderLater(notebook as Snapshot, 90_000);

    expect(screen.getByRole("status")).toHaveTextContent("Leitura antiga · sem atualização há 1 minuto");
    expect(group("CPU")).toHaveTextContent("2%");
    expect(meter("RAM")).toBeInTheDocument();
    expect(document.querySelector(".readings")).toHaveAttribute("data-stale", "true");
  });

  it("does not flag a reading that is still fresh", () => {
    renderLater(notebook as Snapshot, 3_000);

    expect(screen.queryByText(/Leitura antiga/)).not.toBeInTheDocument();
  });
});
