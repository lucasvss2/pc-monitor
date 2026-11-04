# Validação gráfica de Tarefas no Windows 11

Status: claimed
Assignee: Codex / usuário
Parent: ../spec.md

## Escopo

Conectar o Jira pelo aplicativo e alternar Trello/Jira, preservando conexão e quadro. Não alterar cards ou itens reais. Registrar somente categorias e resultados, sem site, conta, quadro, credenciais ou conteúdo dos itens. Remover artefatos temporários obsoletos ao encerrar; preservar dados e conexões úteis ao usuário.

## Comments

Versão experimental da CI 41895134794, código 5269be6, SHA-256 73d2a123411f1eebd836f8ce55b090009d434eb87f1f50f62c02d89fd6186592. Executável sem assinatura, execução direta confirmada pelo usuário, sem instalador e sem mudança de proteção. Uma única instância do executável esperado foi verificada. Preferências existentes com schema 2, inicialização e boas-vindas já concluídas; arquivo de seleção de Tarefas ausente no início, portanto fonte padrão Trello. Âncora inicial registrada apenas como Módulo Agentes, para restauração ao encerrar.

O usuário confirmou abertura do Painel pelo Indicador. A automação Windows não retornou uma janela do monitor mesmo após a abertura; não foram usados handles inventados, automação alternativa de terminal ou capturas brutas. O restante deste aceite é assistido, com ações/credenciais pelo usuário e resultados explicitamente atribuídos às suas confirmações. Conexão pessoal e escolha de quadro ainda em andamento.

O usuário ainda precisa criar seu token e relatou fechamento ao clicar fora. Isso corresponde ao callback nativo Focused(false), que fecha e destrói a webview. Ele respondeu explicitamente que prefere manter sempre aberto e fechar pelo X ou Escape. A revisão foi registrada no ticket jira-windows/10 e no ADR 0006; não se trata de falha de autenticação nem prova de falha no cofre. A conexão Jira e a troca de fontes permanecem pendentes enquanto a nova versão é preparada. Um teste no caminho real do evento nativo, com a ação de fechar injetada, deve reproduzir a regressão sem acessar credenciais nem automatizar autenticação.

A CI 41918310218 reproduziu o evento nativo na revisão 45632ec: losing_focus_does_not_close_the_panel_or_discard_a_connection_draft falhou com switching to another app must keep the panel open; os outros 268 testes Rust Windows passaram. O cenário mínimo é um evento Focused(false) e a observação da ação de fechar; não foram necessários logs de conteúdo, hipóteses de autenticação ou instrumentação adicional. O callback agora aceita fechamento somente em CloseRequested; X mantém o caminho existente, e Escape continua no comando explícito da UI. A mudança não conserva a janela no topo nem captura foco.

O usuário encerrou a versão anterior pelo Indicador, e ausência de processo foi verificada. EXE e pasta temporária antigos removidos. A revisão automática recusou a exclusão recursiva de associações COM/AppUserModelId/protocolo pelo risco de atingir valores de terceiros; nenhuma dessas chaves foi apagada. A preparação da nova versão e a validação gráfica continuam; eventual limpeza dessas associações exige revisão/autorização específica e não foi contornada.
