import { act, render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import TasksSettings from "./TasksSettings.svelte";
import TasksModule from "./TasksModule.svelte";
import type { Core, TasksData, TaskTransition } from "../core";
import full from "./fixtures/full-jira.json";

function core(): Core { return { initialState:vi.fn(), listenSnapshots:vi.fn(), chooseAnchor:vi.fn(), reportVisibleModules:vi.fn(), closePanel:vi.fn() }; }
const data = () => structuredClone(full) as TasksData;
const choices: TaskTransition[] = [{ id:"1",name:"Iniciar",status:"Em andamento",requiresFields:false },{ id:"2",name:"Concluir",status:"Concluído",requiresFields:true }];

it("selects a provider with its expected generation and does not claim completion optimistically", async () => {
  let finish!: () => void;
  const api=core();api.switchTaskProvider=vi.fn(()=>new Promise<void>(resolve=>finish=resolve));
  render(TasksSettings,{data:data(),generation:0,core:api});
  const selector=screen.getByRole("combobox",{name:"Provedor de tarefas"});
  await userEvent.selectOptions(selector,"trello");
  expect(api.switchTaskProvider).toHaveBeenCalledWith(7,"trello");expect(selector).toHaveValue("jira");expect(selector).toBeDisabled();
  await act(()=>finish());expect(selector).not.toBeDisabled();expect(selector).toHaveValue("jira");
});

it("hides source forms during pending cleanup and retries without connecting the destination", async () => {
  const api=core();api.retryTaskCleanup=vi.fn().mockResolvedValue(undefined);api.connectTasks=vi.fn();
  render(TasksSettings,{data:{provider:"trello",contextGeneration:8,state:"cleanupPending",cleanupFailed:true},generation:8,core:api});
  expect(screen.queryByLabelText("API key")).not.toBeInTheDocument();expect(screen.queryByLabelText("Token de API do Jira")).not.toBeInTheDocument();
  expect(screen.getByRole("combobox",{name:"Provedor de tarefas"})).toBeDisabled();
  await userEvent.click(screen.getByRole("button",{name:"Tentar concluir a limpeza"}));expect(api.retryTaskCleanup).toHaveBeenCalledWith(8);expect(api.connectTasks).not.toHaveBeenCalled();
});

it("clears the Jira token immediately and renders only a safe error on vault failure", async () => {
  const api=core();api.connectTasks=vi.fn().mockRejectedValue("keyringUnavailable");
  render(TasksSettings,{data:{provider:"jira",contextGeneration:7,canWrite:false,state:"missingCredential",refreshing:false,account:null,boards:[],board:null,columns:[],issues:[]},generation:7,core:api});
  await userEvent.type(screen.getByLabelText("Site Jira Cloud"),"https://example.atlassian.net");await userEvent.type(screen.getByLabelText("E-mail da conta Atlassian"),"person@example.com");await userEvent.type(screen.getByLabelText("Token de API do Jira"),"PRIVATE_FAKE_TOKEN");
  await userEvent.click(screen.getByRole("button",{name:"Validar e salvar"}));
  expect(api.connectTasks).toHaveBeenCalledWith(7,{provider:"jira",site:"https://example.atlassian.net",email:"person@example.com",token:"PRIVATE_FAKE_TOKEN"});
  expect(screen.getByLabelText("Token de API do Jira")).toHaveValue("");expect(document.body).not.toHaveTextContent("PRIVATE_FAKE_TOKEN");expect(screen.getAllByRole("alert").some(node=>node.textContent?.includes("cofre"))).toBe(true);
});

async function selectIssue(api: Core) {
  render(TasksModule,{data:data(),generation:7,core:api,configure:vi.fn()});
  await userEvent.click(screen.getByRole("button",{name:"Abrir quadro Jira"}));
  const dialog=screen.getByRole("dialog",{name:"Quadro de exemplo"});await userEvent.click(within(dialog).getByRole("button",{name:/^EXM-1 Item de exemplo 1/}));
  return dialog;
}
it("offers the service transitions explicitly and opens Jira for additional required fields", async () => {
  const api=core();api.taskTransitions=vi.fn().mockResolvedValue(choices);api.performTaskAction=vi.fn();api.openTask=vi.fn();
  const dialog=await selectIssue(api);expect(api.taskTransitions).toHaveBeenCalledWith(7,"20000");
  await userEvent.selectOptions(within(dialog).getByRole("combobox",{name:"Transição permitida"}),"2");expect(within(dialog).getByText(/exige campos adicionais/)).toBeInTheDocument();await userEvent.click(within(dialog).getByRole("button",{name:"Continuar no Jira"}));
  expect(api.openTask).toHaveBeenCalledWith(7,"https://example.atlassian.net/browse/EXM-1");expect(api.performTaskAction).not.toHaveBeenCalled();
  expect(within(dialog).getByText("Alta")).toBeInTheDocument();expect(within(dialog).getByText("Pessoa de exemplo")).toBeInTheDocument();expect(within(dialog).getByText("15/01/2030")).toBeInTheDocument();
});
it("submits one simple transition and explains an unconfirmed result without replaying it", async () => {
  const api=core();api.taskTransitions=vi.fn().mockResolvedValue(choices);api.performTaskAction=vi.fn().mockRejectedValue("unconfirmed");
  const dialog=await selectIssue(api);await userEvent.selectOptions(within(dialog).getByRole("combobox",{name:"Transição permitida"}),"1");await userEvent.click(within(dialog).getByRole("button",{name:"Confirmar transição"}));
  expect(api.performTaskAction).toHaveBeenCalledExactlyOnceWith(7,{kind:"transition",item:"20000",transition:"1"});expect(within(dialog).getByRole("status")).toHaveTextContent("Resultado não confirmado");
});
it("a provider change discards the previous board and its transient UI state", async () => {
  const api=core();const {rerender}=render(TasksModule,{data:data(),generation:7,core:api,configure:vi.fn()});expect(screen.getByText("3 itens · 5 colunas")).toBeInTheDocument();
  await rerender({data:{provider:"trello",contextGeneration:8,state:"missingCredential",refreshing:false,account:null,boards:[],board:null,lists:null},generation:8,core:api,configure:vi.fn()});expect(screen.queryByText("3 itens · 5 colunas")).not.toBeInTheDocument();expect(screen.queryByText("Item de exemplo 1")).not.toBeInTheDocument();expect(screen.getByText("Trello",{selector:".source"})).toBeInTheDocument();
});
