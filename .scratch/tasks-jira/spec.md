# Implementação de Tarefas e Jira

Status: ready-for-human

Implementar a etapa Tarefas/Jira da [especificação aprovada](../jira-windows/spec.md), sem declarar o mínimo Windows completo. Contratos normativos: [provedores](../jira-windows/issues/04-provider-contract.md), ADR 0004 e ADR 0007. A solicitação explícita de avançar autoriza esta implementação; cenários gráficos Windows anteriormente adiados continuam pendentes.

Seams já aprovados por AGENTS.md e pela especificação: portas injetáveis do núcleo para HTTP/cofre/arquivos/relógio e snapshots/comandos da UI. Testar comportamento e privacidade nessas fronteiras, com fixtures sanitizadas e variantes sintéticas identificadas; não adicionar seams de E/S paralelos nem alterar contas reais para provar escrita. A autenticação do app é independente do conector deste chat.

Execução: [identidade e protocolo](issues/01-identity-publication.md), [coordenação e provedores](issues/02-providers-recovery.md), [interface e aceite](issues/03-ui-validation.md). CI Windows/Ubuntu porque build scripts locais estão bloqueados por política; não alterar proteções. Remover ferramentas/arquivos temporários ao encerrar.

## Resultado da implementação

Código e testes implementados na branch codex/windows-tasks-jira. Uso em docs/tarefas.md; evidências e roteiro em docs/tarefas-validacao.md. A etapa não é declarada aceita em uso real: falta executar leitura/ação e conferência gráfica com conta/item de teste autorizado no Ubuntu e no Windows. Não foram feitas alterações no quadro real. Os cenários Windows antes adiados continuam pendentes.

A revisão de código 5269be6 passou integralmente na CI 41895134794 nos runners Ubuntu 24.04 e windows-2025: tipos/UI, formatação/Clippy, testes de núcleo e fixtures, cofre Windows sintético e builds de produção sem bundle. A conexão inicial usa token pessoal sem escopos pelo site; o gateway para tokens com escopos não integra esta implementação e está explicitado no guia. Ferramentas/logs temporários e artefato experimental obsoleto foram removidos. Revisão de sanitização e metadados registrada no relatório de validação.
