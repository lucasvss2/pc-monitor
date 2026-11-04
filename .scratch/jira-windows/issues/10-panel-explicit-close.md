# O Painel deve permanecer aberto ao perder foco?

Type: grilling
Status: resolved
Assignee: Codex / usuário
Parent: ../map.md

## Question

Durante a validação gráfica de Tarefas no Windows 11, o usuário informou que clicar fora fechava o aplicativo e dificultava a criação/cópia de um token de API. A regra anterior fechava o Painel completo desfixado ao perder foco, destruindo a webview e seus formulários transitórios.

## Answer

Em 04/11/2026, o usuário escolheu explicitamente **“Manter sempre aberto; fechar pelo X ou Escape”**, em vez de conservar somente Configurações abertas. Essa resposta substitui a regra de fechamento automático do completo desfixado aprovada no ticket 06. A regra do Painel compartilhado passa a ser permanecer aberto ao perder foco em qualquer tela; fixação continua independente e não captura foco. X fecha o Painel, mantendo o Indicador; Escape respeita primeiro diálogos internos e depois fecha o Painel. Sair no Indicador encerra o aplicativo. Sem persistir rascunhos ou tokens para conservar um formulário: a webview permanece viva enquanto o Painel está aberto e é destruída no fechamento explícito.

A mudança vale para a regra compartilhada dos alvos Windows/Ubuntu. Não implementa antecipadamente o reduzido nem o topmost. O aceite nativo de foco e fechamento continua necessário. Registro durável em docs/adr/0006-painel-reduzido-e-fixacao-independentes.md; reprodução e validação em .scratch/tasks-jira/issues/04-windows-graphical.md.
