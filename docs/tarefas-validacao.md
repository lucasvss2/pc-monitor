# Validação de Tarefas

Implementação na branch `codex/windows-tasks-jira`, conforme a [especificação aprovada](../.scratch/jira-windows/spec.md). O incremento compartilha o núcleo e a UI entre Ubuntu e Windows. Não declara o mínimo Windows completo nem encerra seus cenários gráficos adiados.

## Evidências automatizadas

O núcleo é exercitado nas portas de HTTP, cofre, arquivos e relógio, usando os adaptadores reais de Trello/Jira com fixtures sanitizadas e variantes sintéticas. A UI usa snapshots/comandos. A proveniência está em `src-tauri/fixtures/jira/README.md` e `src/lib/tasks/fixtures/README.md`; as respostas do conector não comprovam autenticação pessoal do aplicativo.

| Contrato | Cobertura |
| --- | --- |
| Migração | Identidade/Âncora legada Trello, conexão e quadro mantidos; campos inválidos isolados, arquivo ilegível, schema futuro preservado e falha de substituição atômica. |
| Publicações | Limpeza com data nula, revisão independente do relógio, eventos anteriores ao bootstrap e descarte de outra execução/revisão antiga. |
| Um provedor ativo | Sem requisições à fonte desativada; limpeza oculta dados e bloqueia ambos os provedores em caso de falha. |
| Recuperação | Falha de gravação da intenção, remoção do cache, exclusão do cofre, checkpoint após exclusão e commit final; retomada idempotente após reinício. |
| Leitura Jira | Configuração real de colunas/status; 101 itens em duas páginas, campos opcionais, ciclo de paginação e repetição de páginas rejeitados. |
| Escrita | Transições permitidas por item; campos obrigatórios ou metadados insuficientes abrem o serviço; transição indisponível/unlistada não escreve. |
| Concorrência | Mudança de provedor aguarda escrita; resultado incerto exige leitura e não é reenviado; comandos antigos recusados. |
| Privacidade | Credencial recusada sem cofre, reconexão validada antes de substituir dados inválidos, token apagado do campo, Jira sem projeção em disco e reinício offline sem itens. |
| Tempo e falhas | Cadência/backoff e Retry-After monotônicos, relógio civil retrocedendo, ações bloqueadas sem leitura válida. |

Resultados finais da CI Ubuntu/Windows serão registrados após a conclusão das verificações desta revisão. Não se conta falha de inicialização de workers locais como falha funcional: neste Windows, os testes Vitest locais usam `--pool=threads --maxWorkers=1`; o build Rust é validado na CI, preservando a política de Controle de Aplicativo do host.

## Aceite com conta de teste — pendente nos dois sistemas

Este roteiro exige uma conta e um item de teste com autorização para alteração. Não usar o quadro real do usuário para provar escrita nem compartilhar token. Executar uma versão identificada, sem feature de fixtures; não instalar binários antigos para reproduzir fluxos já corrigidos.

1. Em instalação com Trello configurado, atualizar e conferir conexão, quadro, Âncora e preferências preservados. Mover somente um card de teste autorizado.
2. Escolher Jira, conectar a própria conta pelo aplicativo e selecionar um quadro acessível. Conferir contagem total, colunas e campos com o serviço; tratar falta de permissão como erro, não quadro vazio.
3. Consultar e executar uma transição simples no item de teste. Verificar uma transição que exige campos e concluir somente no Jira. Nenhuma transição deve ser inferida da coluna.
4. Durante escrita, tentar trocar contexto. Conferir espera e resultado; simular perda de resposta apenas em ambiente controlado. Confirmar ausência de reenvio e leitura obrigatória antes de nova escrita.
5. Alternar Trello/Jira e voltar: conexão/quadro preservados, uma fonte coletando por vez e nenhum dado antigo reaparecendo. Desconectar uma fonte e conferir que a outra continua configurada.
6. Offline na mesma execução, conferir marcação da leitura anterior. Reiniciar offline com Jira selecionado e conferir ausência de itens, sem perder a conexão no cofre. Retomar e aguardar leitura válida antes de alterar.
7. Conferir teclado, Escape/retorno de foco, temas e quadro com rolagem na janela nativa. A prévia `tasks=full-jira` permite verificar apresentação sem contas; não comprova integração HTTP.
8. Conferir somente categorias e resultados de privacidade, sem imprimir segredos ou conteúdo do quadro. Remover credenciais de teste pelo aplicativo e arquivos/instalações temporários ao encerrar. Preservar configurações anteriores e dados de terceiros.

Nenhum desses passos com alteração real foi executado nesta implementação. O roteiro anterior de Windows permanece separado em [windows-validacao.md](windows-validacao.md).
