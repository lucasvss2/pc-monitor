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

Revisão de código: `5269be6`, aprovada integralmente na [CI 41895134794](https://github.com/lucasvss2/pc-monitor/actions/runs/41895134794) nos dois sistemas em 04/11/2026. O runner `windows-2025` não substitui o aceite gráfico no Windows 11.

| Verificação | Resultado |
| --- | --- |
| UI no runner Windows | 391 testes passaram em 17 arquivos. |
| Rust no runner Windows | 268 testes unitários e um teste de integração passaram. O teste de cofre inicialmente ignorado foi executado separadamente: passou e removeu a entrada sintética. |
| Fixtures nativas Windows | 13 testes passaram, incluindo a apresentação Jira sem rede/cofre. |
| Produção Windows | Build sem bundle e teste do modo silencioso de hooks no executável GUI passaram. Artefato experimental sem assinatura mantido na execução final, sem instalação no host. |
| Ubuntu 24.04 | 333 testes unitários, três de integração e 16 de fixtures passaram; build de produção sem bundle passou. |
| Ambos os runners | Formatação, Svelte, suíte da UI, build da UI e Clippy com warnings negados, com e sem fixtures, passaram. |
| Host de desenvolvimento | Suíte completa anterior à última regressão: 390 testes; a regressão offline e os outros seis testes de Tarefas passaram depois. Svelte sem erros/avisos e build UI passaram após a correção. |

Não se conta falha de inicialização de workers locais como falha funcional: neste Windows, os testes Vitest locais usam `--pool=threads --maxWorkers=1`; o build Rust é validado na CI, preservando a política de Controle de Aplicativo do host. A [CI 41893790528](https://github.com/lucasvss2/pc-monitor/actions/runs/41893790528) também passou integralmente nos dois runners na revisão `5e537c6`; ela antecede as últimas regressões de paginação/transições e reinício offline, cobertas pela execução final.

## Sanitização e limpeza

Revisados os caminhos alterados em `.scratch/tasks-jira/`, `README.md`, `docs/`, `src-tauri/src/`, `src-tauri/fixtures/jira/` e `src/lib/`: diff completo e índice, strings, JSON e metadados de commits. Não há imagens novas. As oito fixtures JSON Jira foram verificadas estruturalmente: somente hostname de exemplo, nenhum campo de e-mail e identificador de conta sintético. Campos e relações sanitizados, variantes sintéticas e limitações de captura estão nos READMEs das fixtures; nenhuma resposta bruta nem mapa de substituição foi publicado. Autor/committer usam noreply.

O formatador temporário, suas DLLs e os logs locais de teste foram removidos do diretório exclusivo desta tarefa. Não foram instaladas novas versões do aplicativo no host, nem alterados seus hooks ou credenciais. O artefato Windows antigo da revisão `5e537c6` foi removido após a conclusão do artefato final; uma execução intermediária obsoleta foi cancelada e não deixou artefatos. O estado final do Git contém somente os arquivos intencionais versionados.

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

## Rodada assistida Windows 11

O [ticket de validação gráfica](../.scratch/tasks-jira/issues/04-windows-graphical.md) registra o recorte concluído por confirmação assistida. O usuário relatou perda do formulário ao clicar fora e revisou a regra para manter o Painel aberto até X/Escape. A correção `5b4034d` passou integralmente na [CI 41919208165](https://github.com/lucasvss2/pc-monitor/actions/runs/41919208165), incluindo regressão de foco nos dois SOs, 391 testes UI e builds de produção. No Windows 11, o usuário confirmou permanência das Configurações ao perder foco, fechamento/reabertura com X e Escape, autenticação/lista de quadros/leitura Jira e retorno ao mesmo quadro sem credenciais após trocar para Trello. Trello apresentou seu formulário sem conexão: leitura de uma conta Trello, transições reais e aceite Ubuntu continuam pendentes. Credenciais foram informadas somente pelo usuário no aplicativo, sem capturas brutas. O pedido adicional de arraste está no [ticket de reposicionamento](../.scratch/tasks-jira/issues/05-window-drag.md).

Na revisão `c53247a`, o usuário confirmou arraste no Windows, botões Configurações/X funcionando e reabertura pelo Indicador, além de Jira conectado após encerrar o processo e abrir a nova versão. A [CI 41923314348](https://github.com/lucasvss2/pc-monitor/actions/runs/41923314348) passou nos dois sistemas. Apenas a versão atual continua em uso; EXEs/pastas temporárias e artefatos remotos das versões substituídas foram removidos. Associações do Registro em uso foram preservadas após a recusa da exclusão ampla pela revisão automática. O teste comprova movimento atual; persistência de geometrias e validação gráfica Ubuntu continuam separadas.
