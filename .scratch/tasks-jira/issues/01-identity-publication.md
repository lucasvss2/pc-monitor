# Identidade Tarefas, migração e publicações

Status: resolved
Assignee: Codex
Parent: ../spec.md

Migrar Trello para Tarefas preservando cofre, quadro e preferências válidas. Versionar preferências, preservar schema futuro e usar escrita atômica. Publicações com runtime/revision/contextGeneration independentes de updatedAt; bootstrap e eventos concorrentes não restauram dados antigos.

## Comments

A CI 41879807758 confirmou vermelho funcional no Windows: legacy_trello_anchor_serializes_with_the_stable_tasks_identity recebeu trello em vez de tasks; 244 testes passaram. A migração mantém a entrada trello do cofre e aceita o identificador legado ao desserializar. A UI reproduziu a limpeza ignorada por updatedAt nulo; após ordenar por revisão, a mesma regressão passou localmente com pool de threads. O worker por processo não iniciou neste ambiente, portanto esse erro de infraestrutura não foi contado como falha funcional. Próxima regressão: um tema inválido não deve apagar Âncora, movimento reduzido, welcome ou preferências de atualização válidos.

A CI 41884807582 chegou aos testes Rust nos dois SOs e confirmou a regressão de preferências: tema inválido fazia a Âncora voltar para IA. Também apontou expectativas antigas de nome de cache/log, atualizadas para a identidade tasks. A leitura passa a validar campos separadamente, manter campos adicionais do arquivo legado, preservar schema futuro/arquivo ilegível e gravar por temporário+rename. O bootstrap nativo aguarda inscrições no bridge; fixtures antigas continuam aceitas somente para testes da UI. A suíte local completa passou 388 testes; svelte-check sem erros/avisos e build de produção passaram. Validação Rust do incremento completo ainda em andamento.

## Answer

Implementação concluída. A CI 41893790528 passou integralmente no Ubuntu e no Windows, incluindo migração de preferências e guarda do arquivo original em falha atômica. Na revisão final 5269be6, os testes da UI também cobrem execução/revisão antiga, relógio civil retrocedendo e evento recebido durante bootstrap; a CI Windows passou 391 testes de UI. Nenhum dado Jira é usado como relógio de publicação. A entrada trello do cofre e o quadro legado permanecem preservados. Resultados finais e limitações em docs/tarefas-validacao.md.
