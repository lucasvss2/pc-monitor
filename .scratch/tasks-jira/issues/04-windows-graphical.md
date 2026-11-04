# Validação gráfica de Tarefas no Windows 11

Status: resolved
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

Correção de código 5b4034d na CI 41919208165. Os arquivos locais completos de abertura/interação passaram 15 e 58 testes, respectivamente. Uma execução filtrada anterior não foi contada como aceite: houve timeout de worker e uma falha na contagem global de temporizadores; executar os arquivos completos passou. A UI não foi alterada nesta correção nativa. Nenhum log temporário ou instrumentação de conteúdo foi criado. Uma leitura somente de metadados do Registro confirmou destinos apontando ao EXE de teste e ausência de nomes de valores/subchaves inesperados no nível das três chaves; isso não autorizou apagá-las nem inspecionou credenciais. A verificação gráfica da nova regra e a conexão continuam pendentes.

A CI 41919208165 passou integralmente nos dois runners: 391 testes UI; 334 unitários Rust no Ubuntu e 269 no Windows, mais as integrações, fixtures, cofre Windows separado e builds de produção sem bundle. O teste de foco passou nos dois alvos. Máquina de aceite identificada somente como Windows 11 Pro x64, build 26300; não foram coletados hostname, usuário registrado ou identificadores de dispositivo.

O usuário confirmou criação/cópia do token sem transmiti-lo pelo chat. Autorizou a abertura da versão corrigida: 5b4034d, SHA-256 7b230a3076070da4a52968205cabac1fb6a5af4e10ecce1451bf7bae6496b6ea. Executável conferido com o artefato da CI, sem assinatura; uma instância do caminho esperado foi verificada. A inicialização voltou a não expor janela para a automação, portanto foco e conexão continuam em aceite assistido, sem automatizar autenticação.

Aceite assistido confirmado pelo usuário na versão corrigida: Configurações continuaram abertas ao alternar para outro aplicativo e voltar pelo Indicador; autenticação pessoal Jira mostrou conta/lista de quadros; selecionar o quadro e abrir sua apresentação exibiu colunas e itens corretamente, sem erro. A captura fornecida pelo usuário foi inspecionada somente na conversa e não foi copiada para a árvore Git; nomes, chaves e conteúdo privado não entram neste registro. Ela mostra a fonte Jira identificada e uma leitura com mais de uma página de itens, sem comprovar isoladamente igualdade de contagem com o serviço. A troca para Trello e o retorno ao Jira estão em andamento; nenhuma alteração remota em itens foi solicitada.

O usuário confirmou a troca para Trello, que apresentou formulário de chave/token, e o retorno ao Jira com o mesmo quadro sem pedir credenciais. Isso valida o retorno à conexão Jira; não valida autenticação/leitura com uma conta Trello configurada neste Windows. X e Escape também foram confirmados na janela principal: ambos fecharam o Painel, reaberto pelo Indicador com Jira normal. Com Jira ativo, a leitura somente de metadados verificou schema 1, provider jira, nenhuma intenção de limpeza pendente, somente chaves de metadados no arquivo de seleção e ausência de tasks.json/trello.json no cache. Credenciais e conteúdo de itens não foram inspecionados por ferramentas.

EXE local antigo e artefato remoto da versão 5269be6 removidos depois da validação da versão corrigida; versão atual permanece em uso. Novo pedido de reposicionar a janela registrado no ticket 05. Reinício completo, leitura/transição com conta Trello, alterações em itens de teste e aceite Ubuntu continuam fora das confirmações obtidas nesta rodada.

## Resultado

Recorte solicitado de conexão Jira, leitura e troca Jira → Trello → Jira concluído por confirmação assistida, na versão 5b4034d. A fonte Trello foi verificada no estado sem conexão; não se declara leitura real dos dois serviços nem aceite integral da especificação. Permanência ao perder foco e fechamento por X/Escape foram confirmados. O pedido adicional de arraste prossegue no ticket 05, reutilizando a conexão salva e preservando as limitações acima.

Continuação no ticket 05: na versão c53247a, após encerrar e substituir o processo anterior, o usuário confirmou Jira conectado sem pedir token. A persistência da conexão após reinício deixa de ser pendência desta rodada Windows; transições reais, conta Trello conectada e aceite Ubuntu permanecem pendentes. O usuário também confirmou arraste e botões do cabeçalho após o movimento.
