# Identidade Tarefas, migração e publicações

Status: ready-for-agent
Assignee: Codex
Parent: ../spec.md

Migrar Trello para Tarefas preservando cofre, quadro e preferências válidas. Versionar preferências, preservar schema futuro e usar escrita atômica. Publicações com runtime/revision/contextGeneration independentes de updatedAt; bootstrap e eventos concorrentes não restauram dados antigos.

## Comments

A CI 41879807758 confirmou vermelho funcional no Windows: legacy_trello_anchor_serializes_with_the_stable_tasks_identity recebeu trello em vez de tasks; 244 testes passaram. A migração mantém a entrada trello do cofre e aceita o identificador legado ao desserializar. A UI reproduziu a limpeza ignorada por updatedAt nulo; após ordenar por revisão, a mesma regressão passou localmente com pool de threads. O worker por processo não iniciou neste ambiente, portanto esse erro de infraestrutura não foi contado como falha funcional. Próxima regressão: um tema inválido não deve apagar Âncora, movimento reduzido, welcome ou preferências de atualização válidos.
