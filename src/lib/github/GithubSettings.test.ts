import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import GithubSettings from "./GithubSettings.svelte";
import full from "./fixtures/full.json";
import invalid from "./fixtures/invalid-credential.json";
import missing from "./fixtures/missing-gh.json";
import ownToken from "./fixtures/own-token.json";

const FIELD = "Token clássico (escopo repo)";
const INSTRUCTION = "No terminal, execute gh auth login e depois atualize este Módulo.";

async function submitToken(value = "private-token") {
  await fireEvent.input(screen.getByLabelText(FIELD), { target: { value } });
  await fireEvent.click(screen.getByRole("button", { name: "Validar e salvar" }));
}

describe("collapsed and open states", () => {
  it("shows only the state line and 'Usar token próprio' when connected via gh", async () => {
    render(GithubSettings, { data: full, save: vi.fn() });
    expect(screen.getByRole("status")).toHaveTextContent("Conectado via gh · exemplo");
    expect(screen.queryByLabelText(FIELD)).not.toBeInTheDocument();
    expect(screen.queryByText(INSTRUCTION)).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Usar token próprio" }));
    expect(screen.getByLabelText(FIELD)).toBeInTheDocument();
  });

  it("shows the own token state once and 'Trocar token'", async () => {
    render(GithubSettings, { data: ownToken, save: vi.fn() });
    expect(screen.getByRole("status")).toHaveTextContent("Token próprio · exemplo");
    expect(screen.getAllByText(/exemplo/)).toHaveLength(1);
    expect(screen.queryByLabelText(FIELD)).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Trocar token" }));
    expect(screen.getByLabelText(FIELD)).toBeInTheDocument();
  });

  it.each([
    ["without gh", missing, "Não conectado"],
    ["with an invalid credential", invalid, "Credencial do GitHub inválida"],
  ])("opens the form with the gh instruction %s", (_, data, state) => {
    render(GithubSettings, { data, save: vi.fn() });
    expect(screen.getByRole("status")).toHaveTextContent(state);
    expect(screen.getByText(INSTRUCTION)).toBeInTheDocument();
    expect(screen.getByLabelText(FIELD)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Usar token próprio|Trocar token/ })).not.toBeInTheDocument();
  });

  it("collapses after a successful save and shows the new login", async () => {
    const save = vi.fn().mockResolvedValue("novo-login");
    render(GithubSettings, { data: missing, save });
    await submitToken();
    expect(save).toHaveBeenCalledWith("private-token");
    expect(await screen.findByText("Token próprio · novo-login")).toBeInTheDocument();
    expect(screen.queryByLabelText(FIELD)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Trocar token" })).toBeInTheDocument();
  });

  it("lets the next snapshot take over from the saved login", async () => {
    const { rerender } = render(GithubSettings, { data: missing, save: vi.fn().mockResolvedValue("novo-login") });
    await submitToken();
    expect(await screen.findByText("Token próprio · novo-login")).toBeInTheDocument();
    await rerender({ data: ownToken });
    expect(screen.getByRole("status")).toHaveTextContent("Token próprio · exemplo");
    expect(screen.queryByLabelText(FIELD)).not.toBeInTheDocument();
  });

  it("shows the check in progress without a snapshot", () => {
    render(GithubSettings, { data: undefined, save: vi.fn() });
    expect(screen.getByRole("status")).toHaveTextContent("Verificando…");
    expect(screen.queryByLabelText(FIELD)).not.toBeInTheDocument();
  });

  it("collapses after replacing a gh credential with an own token", async () => {
    render(GithubSettings, { data: full, save: vi.fn().mockResolvedValue("exemplo") });
    await fireEvent.click(screen.getByRole("button", { name: "Usar token próprio" }));
    await submitToken();
    expect(await screen.findByText("Token próprio · exemplo")).toBeInTheDocument();
    expect(screen.queryByLabelText(FIELD)).not.toBeInTheDocument();
  });
});

it.each([
  ["invalidToken", "Token inválido"],
  ["missingRepo", "Falta o escopo repo"],
  ["offline", "Sem conexão, tente de novo"],
  ["keyringUnavailable", "Cofre do sistema indisponível; o token não foi salvo."],
])("keeps the form open and shows %s beside the token field", async (error, message) => {
  render(GithubSettings, { data: missing, save: vi.fn().mockRejectedValue(error) });
  await submitToken();
  expect(await screen.findByRole("alert")).toHaveTextContent(message);
  const field = screen.getByLabelText(FIELD);
  expect(field).toHaveAccessibleDescription(message);
  expect(field).toHaveValue("");
  expect(screen.getByRole("status")).toHaveTextContent("Não conectado");
});

it("keeps the field a password and clears the secret after submitting", async () => {
  render(GithubSettings, { data: missing, save: vi.fn().mockRejectedValue("invalidToken") });
  const field = screen.getByLabelText(FIELD);
  expect(field).toHaveAttribute("type", "password");
  await submitToken();
  expect(field).toHaveValue("");
});

it("prevents duplicate submissions while validation is pending", async () => {
  let complete!: (login: string) => void;
  const save = vi.fn(() => new Promise<string>((resolve) => complete = resolve));
  render(GithubSettings, { data: missing, save });
  await submitToken();
  expect(screen.getByRole("button", { name: "Validando…" })).toBeDisabled();
  expect(screen.getByLabelText(FIELD)).toBeDisabled();
  complete("exemplo");
  expect(await screen.findByText("Token próprio · exemplo")).toBeInTheDocument();
  expect(save).toHaveBeenCalledOnce();
});

it("shows the own token account again when settings are reopened, without saving", () => {
  const save = vi.fn();
  render(GithubSettings, { data: ownToken, save }).unmount();
  render(GithubSettings, { data: ownToken, save });
  expect(screen.getByRole("status")).toHaveTextContent("Token próprio · exemplo");
  expect(save).not.toHaveBeenCalled();
});
