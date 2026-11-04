import { render, screen, waitFor, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import AgentSettings from "./AgentSettings.svelte";
import empty from "./fixtures/empty.json";

it("changes hooks only after a click and shows the manual Codex trust step", async () => {
  const setHooks = vi.fn().mockResolvedValue(undefined);
  render(AgentSettings, { data: empty, save: vi.fn(), setHooks });
  expect(setHooks).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("checkbox", { name: "Detecção instantânea" }));
  expect(setHooks).toHaveBeenCalledWith(true);
  expect(screen.getByText(/\/hooks/)).toBeVisible();
  await userEvent.click(screen.getByRole("checkbox", { name: "Detecção instantânea" }));
  expect(setHooks).toHaveBeenLastCalledWith(false);
});
it("keeps the switch off and reports a configuration failure", async () => {
  const setHooks = vi.fn().mockRejectedValue("configurationFailed");
  render(AgentSettings, { data: empty, save: vi.fn(), setHooks });
  await userEvent.click(screen.getByRole("checkbox", { name: "Detecção instantânea" }));
  await waitFor(() => expect(screen.getByRole("alert")).toBeVisible());
  expect(screen.getByRole("checkbox", { name: "Detecção instantânea" })).not.toBeChecked();
});
it("puts instant detection first and groups the three notification switches", () => {
  render(AgentSettings, { data: { ...empty, notificationPreferences: { waiting: true, done: false, failed: true } }, save: vi.fn() });
  const switches = screen.getAllByRole("checkbox");
  expect(switches[0]).toHaveAccessibleName("Detecção instantânea");
  const group = screen.getByRole("group", { name: "Notificações" });
  const [waiting, done, failed] = within(group).getAllByRole("checkbox");
  expect(waiting).toHaveAccessibleName("Aguardando");
  expect(waiting).toBeChecked();
  expect(done).toHaveAccessibleName("Turno concluído");
  expect(done).not.toBeChecked();
  expect(failed).toHaveAccessibleName("Falha");
  expect(failed).toBeChecked();
  expect(within(group).queryByRole("checkbox", { name: "Detecção instantânea" })).not.toBeInTheDocument();
});
it("saves a notification preference only for the switch clicked", async () => {
  const save = vi.fn();
  render(AgentSettings, { data: empty, save });
  await userEvent.click(within(screen.getByRole("group", { name: "Notificações" })).getByRole("checkbox", { name: "Turno concluído" }));
  expect(save).toHaveBeenCalledExactlyOnceWith({ waiting: true, done: false, failed: true });
});
it("states whether instant detection is on", () => {
  const { unmount } = render(AgentSettings, { data: { ...empty, hooksEnabled: true }, save: vi.fn() });
  expect(screen.getByRole("status")).toHaveTextContent("Detecção instantânea ligada");
  unmount();
  render(AgentSettings, { data: empty, save: vi.fn() });
  expect(screen.getByRole("status")).toHaveTextContent("Detecção instantânea desligada");
});
it("warns when Codex cannot report waiting without instant detection", () => {
  render(AgentSettings, { data: { ...empty, codexWaitingUnavailable: true }, save: vi.fn() });
  expect(screen.getByRole("status")).toHaveTextContent("Aguardando indisponível sem Detecção instantânea");
});
it("shows the configuration failure right below the instant detection switch", async () => {
  render(AgentSettings, { data: empty, save: vi.fn(), setHooks: vi.fn().mockRejectedValue("configurationFailed") });
  const toggle = screen.getByRole("checkbox", { name: "Detecção instantânea" });
  await userEvent.click(toggle);
  await waitFor(() => expect(toggle).toHaveAccessibleDescription(/Não foi possível alterar os hooks/));
});

it("explains unavailable native notification delivery without changing saved preferences", () => {
  const save = vi.fn();
  render(AgentSettings, { data: { ...empty, notificationsAvailable: false }, save });
  expect(screen.getByText("Notificações nativas indisponíveis nesta sessão.")).toBeVisible();
  expect(save).not.toHaveBeenCalled();
});
