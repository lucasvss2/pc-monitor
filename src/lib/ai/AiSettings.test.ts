import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { expect, it, vi } from "vitest";
import AiSettings from "./AiSettings.svelte";
import type { AiData } from "../core";
import off from "./fixtures/claude-off.json";
import connected from "./fixtures/claude-connected.json";
import replaced from "./fixtures/claude-replaced.json";
import usage from "./fixtures/claude-usage.json";
import expired from "./fixtures/claude-expired.json";
import claudeMissing from "./fixtures/claude-missing.json";
import missing from "./fixtures/missing.json";

const block = (name: "Claude" | "Codex") => screen.getByRole("region", { name });
const props = (data: unknown) => ({ data: data as AiData, connect: vi.fn(), disconnect: vi.fn(), setUsage: vi.fn() });

it("explains unsupported Windows OAuth without offering an opt-in", () => {
  render(AiSettings, props({ ...off, claude: { ...off.claude, usage: "unsupported" } }));
  expect(screen.getByText("OAuth de uso indisponível no Windows nesta versão. Use a conexão pela status line.")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Ler via oauth/usage…" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Conectar ao Claude Code" })).toBeInTheDocument();
});

it("does not confirm Codex limits before the first reading", () => {
  render(AiSettings, props({ ...off, codex: { ...off.codex, state: "empty", reading: null } }));
  expect(within(block("Codex")).getByText("Sem leitura dos limites do Codex ainda · use o Codex ou atualize com ↻")).toBeInTheDocument();
  expect(within(block("Codex")).queryByText("Codex detectado · plano plus")).not.toBeInTheDocument();
});


it("splits the section into a Claude and a Codex block, each with its own state", () => {
  render(AiSettings, props(connected));
  expect(within(block("Claude")).getByText("Conectado ao Claude Code pela status line")).toBeInTheDocument();
  expect(within(block("Codex")).getByText("Codex detectado · plano plus")).toBeInTheDocument();
  expect(within(block("Codex")).queryByRole("button")).not.toBeInTheDocument();
});

it("shows both providers as not found", () => {
  render(AiSettings, props({ ...missing, claude: claudeMissing.claude }));
  expect(within(block("Claude")).getByText("Claude Code não encontrado · instale e faça login no CLI")).toBeInTheDocument();
  expect(within(block("Codex")).getByText("Codex não encontrado · instale e faça login no CLI")).toBeInTheDocument();
});

it("shows both providers as being checked before the first snapshot", () => {
  render(AiSettings, props(undefined));
  expect(within(block("Claude")).getByText("Verificando…")).toBeInTheDocument();
  expect(within(block("Codex")).getByText("Verificando…")).toBeInTheDocument();
});

it("connects to Claude Code only on click", async () => {
  const connect = vi.fn().mockResolvedValue(undefined);
  render(AiSettings, { ...props(off), connect });
  expect(connect).not.toHaveBeenCalled();
  await fireEvent.click(screen.getByRole("button", { name: "Conectar ao Claude Code" }));
  expect(connect).toHaveBeenCalledOnce();
});

it("disconnects when connected", async () => {
  const disconnect = vi.fn().mockResolvedValue(undefined);
  render(AiSettings, { ...props(connected), disconnect });
  expect(screen.getByText("Conectado ao Claude Code pela status line")).toBeInTheDocument();
  await fireEvent.click(screen.getByRole("button", { name: "Desconectar" }));
  expect(disconnect).toHaveBeenCalledOnce();
});

it("offers to reconnect after another tool replaced the status line", async () => {
  const connect = vi.fn().mockResolvedValue(undefined);
  render(AiSettings, { ...props(replaced), connect });
  expect(within(block("Claude")).getByRole("status")).toHaveTextContent(/^Status line desconectada: outra ferramenta trocou/);
  expect(screen.getByRole("button", { name: "Desconectar" })).toBeInTheDocument();
  await fireEvent.click(screen.getByRole("button", { name: "Reconectar" }));
  expect(connect).toHaveBeenCalledOnce();
});

it("explains settings it refused to change", async () => {
  render(AiSettings, { ...props(off), connect: vi.fn().mockRejectedValue("invalidSettings") });
  await fireEvent.click(screen.getByRole("button", { name: "Conectar ao Claude Code" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("~/.claude/settings.json não é um JSON válido; nada foi alterado");
});

it("turns oauth/usage on only after the warning is confirmed", async () => {
  const setUsage = vi.fn().mockResolvedValue(undefined);
  render(AiSettings, { ...props(off), setUsage });
  const toggle = () => screen.getByRole("button", { name: "Ler via oauth/usage…" });
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  await fireEvent.click(toggle());
  const warning = screen.getByRole("alertdialog", { name: "Endpoint não documentado" });
  expect(warning).toHaveTextContent("não documentado");
  expect(warning).toHaveTextContent("zona cinzenta nos termos da Anthropic");
  expect(warning).toHaveTextContent("nunca renova");
  expect(setUsage).not.toHaveBeenCalled();
  await fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  expect(setUsage).not.toHaveBeenCalled();
  await fireEvent.click(toggle());
  await fireEvent.click(screen.getByRole("button", { name: "Entendi o risco, ligar" }));
  expect(setUsage).toHaveBeenCalledExactlyOnceWith(true);
});

it("turns oauth/usage off at once", async () => {
  const setUsage = vi.fn().mockResolvedValue(undefined);
  render(AiSettings, { ...props(usage), setUsage });
  expect(screen.getByText("Leitura via oauth/usage ligada · a cada 5 min")).toBeInTheDocument();
  await fireEvent.click(screen.getByRole("button", { name: "Desligar oauth/usage" }));
  expect(setUsage).toHaveBeenCalledExactlyOnceWith(false);
});

it("explains an expired token in Settings", () => {
  render(AiSettings, props(expired));
  expect(screen.getByText("Token do Claude expirado · abra o Claude Code")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Desligar oauth/usage" })).toBeInTheDocument();
});

it("shows a status line not yet connected with the explanation and Connect", () => {
  render(AiSettings, props(off));
  expect(within(block("Claude")).getByText("Status line não conectada")).toBeInTheDocument();
  expect(screen.getByText(/continua rodando a sua status line atual/)).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Desconectar" })).not.toBeInTheDocument();
});

it("shows a failed status line action right below its buttons, not below the opt-in", async () => {
  render(AiSettings, { ...props(connected), disconnect: vi.fn().mockRejectedValue("unwritable") });
  await fireEvent.click(screen.getByRole("button", { name: "Desconectar" }));
  const alert = await screen.findByRole("alert");
  expect(alert).toHaveTextContent("Não foi possível gravar a configuração do Claude Code");
  expect(screen.getByRole("button", { name: "Desconectar" })).toHaveAccessibleDescription(alert.textContent!);
});

it("shows a failed opt-in change right below the opt-in", async () => {
  render(AiSettings, { ...props(usage), setUsage: vi.fn().mockRejectedValue("unwritable") });
  const connect = screen.getByRole("button", { name: "Conectar ao Claude Code" });
  await fireEvent.click(screen.getByRole("button", { name: "Desligar oauth/usage" }));
  const alert = await screen.findByRole("alert");
  expect(connect).not.toHaveAccessibleDescription(alert.textContent!);
  expect(screen.getByRole("button", { name: "Desligar oauth/usage" })).toHaveAccessibleDescription(alert.textContent!);
});

it("still names a connected status line when Claude Code is not found", async () => {
  const disconnect = vi.fn().mockResolvedValue(undefined);
  render(AiSettings, { ...props({ ...claudeMissing, claude: { ...claudeMissing.claude, statusLine: "connected" } }), disconnect });
  expect(within(block("Claude")).getByRole("status")).toHaveTextContent("Claude Code não encontrado");
  expect(within(block("Claude")).getByText("Conectado ao Claude Code pela status line")).toBeInTheDocument();
  await fireEvent.click(screen.getByRole("button", { name: "Desconectar" }));
  expect(disconnect).toHaveBeenCalledOnce();
});

it("keeps an opt-in error when a status line action then succeeds", async () => {
  render(AiSettings, { ...props(usage), connect: vi.fn().mockResolvedValue(undefined), setUsage: vi.fn().mockRejectedValue("unwritable") });
  await fireEvent.click(screen.getByRole("button", { name: "Desligar oauth/usage" }));
  await screen.findByRole("alert");
  await fireEvent.click(screen.getByRole("button", { name: "Conectar ao Claude Code" }));
  expect(screen.getByRole("button", { name: "Desligar oauth/usage" })).toHaveAccessibleDescription("Não foi possível gravar a configuração do Claude Code");
});
