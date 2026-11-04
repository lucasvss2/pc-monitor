# Implementação de Tarefas e Jira

Status: ready-for-agent

Implementar a etapa Tarefas/Jira da [especificação aprovada](../jira-windows/spec.md), sem declarar o mínimo Windows completo. Contratos normativos: [provedores](../jira-windows/issues/04-provider-contract.md), ADR 0004 e ADR 0007. A solicitação explícita de avançar autoriza esta implementação; cenários gráficos Windows anteriormente adiados continuam pendentes.

Seams já aprovados por AGENTS.md e pela especificação: portas injetáveis do núcleo para HTTP/cofre/arquivos/relógio e snapshots/comandos da UI. Testar comportamento e privacidade nessas fronteiras, com fixtures sanitizadas e variantes sintéticas identificadas; não adicionar seams de E/S paralelos nem alterar contas reais para provar escrita. A autenticação do app é independente do conector deste chat.

Execução: [identidade e protocolo](issues/01-identity-publication.md), [coordenação e provedores](issues/02-providers-recovery.md), [interface e aceite](issues/03-ui-validation.md). CI Windows/Ubuntu porque build scripts locais estão bloqueados por política; não alterar proteções. Remover ferramentas/arquivos temporários ao encerrar.
