import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import TrelloSettings from "./TrelloSettings.svelte";
import type { TrelloData } from "../core";
import emptyJson from "./fixtures/empty.json";
import chooseJson from "./fixtures/choose-board.json";
import fullJson from "./fixtures/full.json";
import invalidJson from "./fixtures/invalid-credential.json";

function props(data: unknown, overrides = {}) {
  return {
    data: data as TrelloData,
    connect: vi.fn().mockResolvedValue("exemplo"),
    selectBoard: vi.fn().mockResolvedValue(undefined),
    disconnect: vi.fn().mockResolvedValue(undefined),
    openAdmin: vi.fn(),
    authorize: vi.fn(),
    ...overrides,
  };
}

async function fill(key: string, token: string) {
  await fireEvent.input(screen.getByLabelText("API key"), { target: { value: key } });
  await fireEvent.input(screen.getByLabelText("Token"), { target: { value: token } });
}

it("generates the token with the pasted key", async () => {
  const p = props(emptyJson);
  render(TrelloSettings, p);
  const generate = screen.getByRole("button", { name: "Gerar token" });
  expect(generate).toBeDisabled();
  await fireEvent.input(screen.getByLabelText("API key"), { target: { value: "0123abcd" } });
  await fireEvent.click(generate);
  expect(p.authorize).toHaveBeenCalledWith("0123abcd");
  await fireEvent.click(screen.getByRole("button", { name: "Abrir trello.com/apps/admin" }));
  expect(p.openAdmin).toHaveBeenCalledOnce();
});

it("validates key and token, then clears the secret and opens the board list", async () => {
  const p = props(emptyJson);
  const { rerender } = render(TrelloSettings, p);
  expect(screen.getByLabelText("Token")).toHaveAttribute("type", "password");
  await fill("0123abcd", "private-token");
  await fireEvent.click(screen.getByRole("button", { name: "Validar e salvar" }));
  expect(p.connect).toHaveBeenCalledWith("0123abcd", "private-token");
  expect(await screen.findByRole("status")).toHaveTextContent("Conta exemplo · escolha um quadro");
  expect(screen.queryByLabelText("Token")).not.toBeInTheDocument();
  await rerender({ ...p, data: chooseJson as TrelloData });
  expect(screen.getByRole("radiogroup", { name: "Quadro" })).toBeInTheDocument();
});

it("keeps the board list open after validating when the account keeps its board", async () => {
  const p = props(emptyJson);
  const { rerender } = render(TrelloSettings, p);
  await fill("0123abcd", "private-token");
  await fireEvent.click(screen.getByRole("button", { name: "Validar e salvar" }));
  await rerender({ ...p, data: fullJson as TrelloData });
  expect(screen.getByRole("status")).toHaveTextContent("Conta exemplo · quadro Quadro 1");
  expect(screen.getByRole("radio", { name: "Quadro 1" })).toBeChecked();
});

it.each([
  ["invalidCredential", "Key ou token inválido"],
  ["offline", "Sem conexão, tente de novo"],
  ["keyringUnavailable", "Cofre do sistema indisponível; a key e o token não foram salvos."],
])("shows %s beside the fields", async (error, message) => {
  render(TrelloSettings, props(emptyJson, { connect: vi.fn().mockRejectedValue(error) }));
  await fill("0123abcd", "private-token");
  await fireEvent.click(screen.getByRole("button", { name: "Validar e salvar" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(message);
  expect(screen.getByLabelText("Token")).toHaveAccessibleDescription(message);
  expect(screen.getByRole("status")).toHaveTextContent("Não conectado");
});

it("chooses among the open boards without a native popup", async () => {
  // A <select> opens a GTK popup that takes focus and closes the panel under it.
  const p = props(chooseJson);
  const { container } = render(TrelloSettings, p);
  expect(container.querySelector("select")).toBeNull();
  const board = (chooseJson as TrelloData).boards[1];
  const group = screen.getByRole("radiogroup", { name: "Quadro" });
  await fireEvent.click(within(group).getByRole("radio", { name: board.name }));
  expect(p.selectBoard).toHaveBeenCalledWith(board.id);
});

it("disconnects from the connected summary", async () => {
  const p = props(fullJson);
  const { rerender } = render(TrelloSettings, p);
  await fireEvent.click(screen.getByRole("button", { name: "Desconectar" }));
  expect(p.disconnect).toHaveBeenCalledOnce();
  expect(await screen.findByRole("status")).toHaveTextContent("Não conectado");
  expect(screen.getByLabelText("API key")).toBeInTheDocument();
  await rerender({ ...p, data: emptyJson as TrelloData });
  expect(screen.getByRole("status")).toHaveTextContent("Não conectado");
});

it("hides disconnect and the board selector without an account", () => {
  render(TrelloSettings, props(emptyJson));
  expect(screen.queryByRole("button", { name: "Desconectar" })).not.toBeInTheDocument();
  expect(screen.queryByRole("radiogroup", { name: "Quadro" })).not.toBeInTheDocument();
});

it("explains when the keyring refuses to disconnect", async () => {
  render(TrelloSettings, props(fullJson, { disconnect: vi.fn().mockRejectedValue("keyringUnavailable") }));
  const button = screen.getByRole("button", { name: "Desconectar" });
  await fireEvent.click(button);
  expect(await screen.findByRole("alert")).toHaveTextContent("Não foi possível apagar a key e o token");
  expect(button).toHaveAccessibleDescription(/Não foi possível apagar a key e o token/);
  expect(screen.getByRole("status")).toHaveTextContent("Conta exemplo · quadro Quadro 1");
});

it("restores the pending key when the first snapshot arrives after the section opens", async () => {
  const p = props(undefined);
  const { rerender } = render(TrelloSettings, p);
  await rerender({ ...p, data: { ...emptyJson, pendingKey: "0123abcd" } as TrelloData });
  expect(screen.getByLabelText("API key")).toHaveValue("0123abcd");
});

it("keeps the key typed before generating the token when the panel reopens", async () => {
  render(TrelloSettings, props({ ...emptyJson, pendingKey: "0123abcd" }));
  expect(screen.getByLabelText("API key")).toHaveValue("0123abcd");
  expect(screen.getByRole("button", { name: "Gerar token" })).toBeEnabled();
});

describe("collapsed and open states", () => {
  it("shows only the account and board summary when connected", () => {
    render(TrelloSettings, props(fullJson));
    expect(screen.getByRole("status")).toHaveTextContent("Conta exemplo · quadro Quadro 1");
    expect(screen.queryByLabelText("API key")).not.toBeInTheDocument();
    expect(screen.queryByText(/^1\./)).not.toBeInTheDocument();
    expect(screen.queryByRole("radiogroup", { name: "Quadro" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Trocar quadro" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Desconectar" })).toBeInTheDocument();
  });

  it("shows the numbered key and token steps without a credential", () => {
    render(TrelloSettings, props(emptyJson));
    expect(screen.getByRole("status")).toHaveTextContent("Não conectado");
    const key = screen.getByText(/^1\. /);
    const token = screen.getByText(/^2\. /);
    expect(key.compareDocumentPosition(token) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Trocar quadro" })).not.toBeInTheDocument();
  });

  it("opens the board list without the steps while a board is missing", () => {
    render(TrelloSettings, props(chooseJson));
    expect(screen.getByRole("status")).toHaveTextContent("Conta exemplo · escolha um quadro");
    expect(screen.getByRole("radiogroup", { name: "Quadro" })).toBeInTheDocument();
    expect(screen.queryByLabelText("API key")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Trocar quadro" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Desconectar" })).toBeInTheDocument();
  });

  it("collapses the list once a board is chosen", async () => {
    const p = props(chooseJson);
    render(TrelloSettings, p);
    await fireEvent.click(screen.getByRole("radio", { name: "Quadro extra" }));
    expect(p.selectBoard).toHaveBeenCalledWith("0000000000000000000000b2");
    expect(await screen.findByRole("status")).toHaveTextContent("Conta exemplo · quadro Quadro extra");
    expect(screen.queryByRole("radiogroup", { name: "Quadro" })).not.toBeInTheDocument();
  });

  it("opens the board list with 'Trocar quadro' and collapses it on a new choice", async () => {
    const p = props(fullJson);
    render(TrelloSettings, p);
    const changeBoard = screen.getByRole("button", { name: "Trocar quadro" });
    expect(changeBoard).toHaveAttribute("aria-expanded", "false");
    await fireEvent.click(changeBoard);
    expect(changeBoard).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("radio", { name: "Quadro 1" })).toBeChecked();
    await fireEvent.click(screen.getByRole("radio", { name: "Quadro extra" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Conta exemplo · quadro Quadro extra");
    expect(screen.queryByRole("radiogroup", { name: "Quadro" })).not.toBeInTheDocument();
  });

  it("keeps the list open with the error beside it when choosing fails", async () => {
    render(TrelloSettings, props(chooseJson, { selectBoard: vi.fn().mockRejectedValue("offline") }));
    await fireEvent.click(screen.getByRole("radio", { name: "Quadro extra" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Sem conexão, tente de novo");
    expect(screen.getByRole("radiogroup", { name: "Quadro" })).toHaveAccessibleDescription("Sem conexão, tente de novo");
    expect(screen.getByRole("radio", { name: "Quadro extra" })).not.toBeChecked();
  });

  it("keeps the current board checked when switching fails", async () => {
    render(TrelloSettings, props(fullJson, { selectBoard: vi.fn().mockRejectedValue("offline") }));
    await fireEvent.click(screen.getByRole("button", { name: "Trocar quadro" }));
    await fireEvent.click(screen.getByRole("radio", { name: "Quadro extra" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Sem conexão, tente de novo");
    expect(screen.getByRole("radio", { name: "Quadro 1" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Quadro extra" })).not.toBeChecked();
  });

  it("hides the previous credential's boards until the snapshot after validating arrives", async () => {
    const p = props(invalidJson, { connect: vi.fn().mockResolvedValue("outra-conta") });
    const { rerender } = render(TrelloSettings, p);
    await fill("0123abcd", "private-token");
    await fireEvent.click(screen.getByRole("button", { name: "Validar e salvar" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Conta outra-conta · escolha um quadro");
    expect(screen.queryByRole("radiogroup", { name: "Quadro" })).not.toBeInTheDocument();
    await rerender({ ...p, data: chooseJson as TrelloData });
    expect(screen.getByRole("radiogroup", { name: "Quadro" })).toBeInTheDocument();
  });

  it("asks for key and token again when the credential is invalid", () => {
    render(TrelloSettings, props(invalidJson));
    expect(screen.getByRole("status")).toHaveTextContent("Credencial inválida");
    expect(screen.getByLabelText("API key")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Desconectar" })).toBeInTheDocument();
  });
});
