# Tarefas: Trello ou Jira Cloud

O Módulo **Tarefas** mantém a mesma posição e Âncora ao escolher outra fonte. **Trello** ou **Jira** aparece no cabeçalho. Um único provedor coleta por vez. Esta funcionalidade está na árvore de desenvolvimento; a aceitação com conta de teste nos dois sistemas ainda deve ser registrada antes de declarar a etapa validada em uso real.

## Conectar e escolher um quadro

Abra **Configurações → Tarefas** e selecione o provedor. No Trello, use chave e token da API e escolha um quadro. Ao atualizar uma instalação existente, Trello continua selecionado, com conexão e quadro preservados; a Âncora antiga passa a se chamar Tarefas.

Para Jira, a integração inicial atende **Jira Cloud**, no endereço raiz HTTPS de um site `*.atlassian.net`. Instalações próprias e domínios personalizados ficam fora deste recorte. Informe site, e-mail da conta e um token pessoal de API compatível com autenticação básica no próprio site. A [documentação da Atlassian](https://developer.atlassian.com/cloud/jira/platform/basic-auth-for-rest-apis/) explica como criar o token. Não cole credenciais em tickets, logs ou chats. O token é apagado do campo ao enviar; se a conexão falhar, será necessário informá-lo novamente.

O aplicativo valida a conta e lista seus quadros acessíveis. Selecione o quadro desejado. A leitura segue o conteúdo e as colunas que o serviço define para esse quadro, incluindo todas as páginas; não usa uma consulta genérica do projeto. Restrições de permissão e configuração do Jira continuam valendo. O acesso concedido a conectores de outros aplicativos não conecta o monitor.

Este fluxo inicial usa um **token pessoal sem escopos**, pela opção **Create API token**. Tokens com escopos usam o gateway `api.atlassian.com/ex/jira/{cloudId}` e não são atendidos pelo acesso direto ao site implementado aqui. A distinção e os dois caminhos de criação estão no [guia de tokens da Atlassian](https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/).

Site, conta, credencial e seleção ficam no cofre nativo: GNOME Keyring no Ubuntu ou Windows Credential Manager no Windows. Sem cofre disponível, a conexão não é salva; não existe armazenamento alternativo em arquivo. Expiração ou revogação do token exige **Atualizar conexão**.

## Consultar e alterar

**Abrir quadro Jira** apresenta as colunas e os itens acessíveis. Selecione um item para ver chave, título, estado e, quando fornecidos, tipo, prioridade, responsável e prazo. Descrições, comentários e anexos não fazem parte da leitura. **Abrir item no Jira** usa o navegador padrão.

As transições são consultadas para cada item. Escolha uma e confirme: o aplicativo não deduz a ação a partir do nome da coluna. Se houver campos obrigatórios que o monitor não preenche, **Continuar no Jira** abre o item para concluir no serviço. Uma recusa não é apresentada como sucesso.

Enquanto uma alteração aguarda resposta, a escolha de provedor, conta ou quadro fica bloqueada. Se a resposta se perder ou expirar, o aplicativo informa **Resultado não confirmado**: a ação pode ter sido aplicada. Ele não a reenvia automaticamente e exige uma leitura válida antes de outra alteração. Essa proteção também vale ao voltar à fonte ou reiniciar o aplicativo. Confira o estado no serviço antes de repetir.

## Trocar, desconectar e recuperar

Trocar o provedor preserva a conexão e o quadro de cada fonte para facilitar o retorno. O aplicativo oculta os dados anteriores, interrompe a coleta da origem, limpa seu cache e só então conclui a troca. **Desconectar** também remove do cofre a conexão e a seleção do provedor ativo; a outra fonte permanece preservada. A desconexão local não revoga um token no serviço.

Se a limpeza ou a gravação da escolha falhar, os dados ficam ocultos e a coleta pausada. Abra Configurações de Tarefas e use **Tentar concluir a limpeza**. O aplicativo preserva uma intenção mínima de recuperação, sem site, conta, quadro, itens ou credenciais; ao reiniciar, resolve essa pendência antes de restaurar cache ou coletar. Não declara a troca concluída enquanto a limpeza estiver pendente.

No Jira, a última leitura existe somente em memória durante a execução. Offline, ela pode continuar visível como leitura anterior, sem permitir alterações. Depois de reiniciar sem conexão, o aplicativo informa que ainda não há leitura disponível. Trello mantém seu cache sanitizado enquanto ativo, sem títulos de cards; esse cache é removido ao trocar a fonte. Cache, logs e notificações nunca recebem conteúdo de itens Jira ou credenciais.

## Limites e validação

A leitura usa as [APIs de quadro do Jira Software Cloud](https://developer.atlassian.com/cloud/jira/software/rest/api-group-board/) e as [transições de itens](https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/), com espera limitada, paginação e respeito a limites de requisição. Erros de leitura não são tratados como quadro vazio. Ranking, edição de campos, comentários e administração de sprints/backlog ficam fora deste fluxo.

As provas automatizadas usam portas injetáveis e fixtures sanitizadas. Elas não substituem o aceite de autenticação pessoal, leitura e ação em conta de teste no Ubuntu e no Windows. O roteiro e as evidências estão em [Validação de Tarefas](tarefas-validacao.md). O suporte completo Windows e seus cenários gráficos anteriormente adiados continuam em [Validação da base Windows](windows-validacao.md).
