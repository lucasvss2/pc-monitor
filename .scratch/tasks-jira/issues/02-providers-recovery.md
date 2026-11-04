# Provedores e troca recuperável

Status: ready-for-human
Assignee: Codex
Parent: ../spec.md

Um provedor ativo. Jira Cloud pessoal, quadro completo paginado, campos acordados, transições por item e navegador quando exigir campos. Intenção durável de troca/desconexão, limpeza verificável e idempotente, bloqueio de ações por geração, timeout incerto sem reenvio e primeira leitura obrigatória; conteúdo Jira só em memória. Trello preservado.

## Comments

Implementação usa um mailbox para ambos os provedores: HTTP síncrono limitado pelo adaptador, comandos com geração esperada e nenhum polling da fonte inativa. Tarefas detém cache e intenção durável de limpeza; os provedores têm leitura em memória e credenciais no cofre. A entrada trello existente é mantida. Jira usa o endpoint enhanced do quadro, mapeia status às colunas e consulta transições com campos; não usa JQL de projeto nem infere modalidade pelo tipo simple. Clientes recusam redirects e desabilitam retry HTTP, inclusive para escritas. Jira tem limites de paginação, tempo, backoff/Retry-After e agendamento monotônico.

As fixtures Jira foram projetadas e sanitizadas em memória antes da gravação, a partir de consultas de leitura do conector. O modo compacto do conector não expunha IDs de status suficientes: a captura foi refeita com campos explícitos e view full, e as relações foram verificadas. Nenhuma escrita foi feita no quadro real, e a autenticação do aplicativo não foi exercitada pelo conector. Procedência e variantes em src-tauri/fixtures/jira/README.md.

Implementação concluída; pendente somente o aceite com conta/item de teste autorizado nos dois sistemas. Testes adicionais verificam a escrita mantendo a troca na fila, exclusão do cofre concluída antes de checkpoint falho, reconexão com dado antigo inválido, repetição de páginas e transições indisponíveis/metadados de campos insuficientes. A CI 41893790528 passou nos dois sistemas; a revisão final 5269be6 acrescenta reinício offline com conexão preservada. Não houve escrita em Jira/Trello real nem modificação de credenciais/hooks desta máquina. O roteiro está em docs/tarefas-validacao.md.
