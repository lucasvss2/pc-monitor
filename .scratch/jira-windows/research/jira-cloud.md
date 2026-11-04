# Jira Cloud: contratos para o Módulo local

Data da pesquisa: **03/11/2026**. Escopo: app pessoal desktop, sem servidor próprio e sem custo adicional pretendido. Pesquisa documental pública; nenhuma credencial, conta, item ou configuração privada foi consultada e nenhuma operação foi executada no Jira.

## Evidência disponível e conclusão

O ticket e a projeção fornecida pela investigação raiz informam uma inspeção anterior do board alvo: `type: simple`, cinco colunas e categorias `[new, indeterminate, indeterminate, indeterminate, done]`. São contexto fornecido, não uma captura refeita nesta pesquisa. O board foi localizado por referências ao projeto; seu nome não deve ser inferido do nome do repositório. Nenhum nome, domínio, identificador ou conteúdo privado é necessário neste relatório.

**Fato documental:** a Atlassian registra que boards de projetos team-managed retornam `simple`, mesmo podendo habilitar sprints e backlog. Seu workaround oficial consulta `GET /rest/agile/1.0/board/{boardId}/features`: sem ambos, Kanban; só backlog, Kanplan; ambos, Scrum. O registro público permanece sem resolução. Portanto, `simple` sozinho não determina a modalidade operacional. [Issue oficial JRACLOUD-85696](https://jira.atlassian.com/browse/JRACLOUD-85696).

**Inferência:** há contratos REST públicos para leitura e transições, mas a pesquisa não comprova que cada endpoint retornará o contrato esperado para o board alvo usando a autenticação própria do monitor. Cinco colunas não demonstram cinco status, nem cinco transições disponíveis em todos os itens. A implementação deve validar capacidades do contexto escolhido; a decisão de autenticação permanece aberta.

## Board, projeto e seleção

**Fatos documentais:** os endpoints abaixo pertencem à API Jira Software. [Referência Board](https://developer.atlassian.com/cloud/jira/software/rest/api-group-board/).

| Finalidade | Contrato atual |
| --- | --- |
| Selecionar | `GET /rest/agile/1.0/board`: boards visíveis; `startAt`, `maxResults`, filtros por nome/projeto. |
| Identificar | `GET /rest/agile/1.0/board/{boardId}`. |
| Configurar leitura | `GET /rest/agile/1.0/board/{boardId}/configuration`: filtro, localização, `columnConfig.columns` ordenadas e seus status; última coluna mapeada representa conclusão; informação de ranking. |
| Ler itens | `GET /rest/software/1.0/board/{boardId}/issue`: versão enhanced; `nextPageToken`, `maxResults`, `fields`, `jql`, `reconcileIssues`; ordem padrão por rank. Só itens visíveis e status mapeados; sem acesso ao board pode retornar vazio. |
| Evitar em integração nova | `GET /rest/agile/1.0/board/{boardId}/issue` está marcado Deprecated. |

Scopes documentados: listagem usa `read:board-scope:jira-software` e `read:project:jira`; configuração usa `read:board-scope.admin:jira-software` e `read:project:jira`; leitura enhanced usa `read:board-scope:jira-software` e `read:issue-details:jira`. [Referência Board](https://developer.atlassian.com/cloud/jira/software/rest/api-group-board/).

**Fato documental:** a API de projetos oferece `GET /rest/api/3/project/search` paginado; o antigo `GET /rest/api/3/project` está deprecated. `GET /rest/api/3/project/{projectIdOrKey}/statuses` entrega status válidos agrupados por tipo de item, com permissão Browse Projects. [Referência Projects](https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-projects/).

**Inferência:** projeto e board não são identificadores intercambiáveis. Uma busca `project = DEMO` pode ser útil como visão de projeto, mas não comprova reprodução do board. A lista de status do projeto não fornece a ordem das cinco colunas nem resolve sua relação com elas. A pesquisa pública não demonstrou uma restrição explícita de `configuration` a `scrum`/`kanban`, nem garante sua resposta para o `simple` alvo.

**Proposta:** escolher explicitamente site e board; guardar contexto composto por provedor/site/board, sem inferir identidade por nome. Exibir erro de acesso ou capacidade separadamente de “nenhum item”. Se a configuração não vier, não inventar cinco colunas a partir de status ou categorias. Permitir uma visão de projeto somente como capacidade separada e assumida pelo produto.

## Busca, paginação e projeção

**Fatos documentais:** a busca atual da plataforma é `GET /rest/api/3/search/jql` ou `POST /rest/api/3/search/jql`; as variantes antigas `/rest/api/3/search` estão em remoção. A versão enhanced aceita `nextPageToken`, `maxResults`, `fields`, `jql` e `reconcileIssues`; resultados recentes podem demorar a aparecer, e `reconcileIssues` reforça consistência após escrita. A visibilidade exige Browse Projects e, quando configurada, segurança do item. [Referência Issue search](https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issue-search/).

**Proposta:** seguir o token opaco retornado até o fim; não convertê-lo em offset, não presumir que uma página curta encerra toda busca e não depender de total exato para desenhar o Painel. Reiniciar a sequência se filtros/contexto mudarem e evitar unir páginas de consultas distintas. Projetar apenas os campos necessários: `id`, `key`, `summary`, `status`, `issuetype` e, se exibido, `project`. O título continua sendo conteúdo privado: pode existir em memória para a UI, mas não em logs, cache em disco ou notificações, conforme `AGENTS.md` e `docs/agents/sanitizacao.md`.

Não há fixture Jira criada por esta pesquisa. Os nomes `DEMO`, os placeholders de rota e exemplos propostos aqui são sintéticos.

## Mudança de estado e campos exigidos

**Fatos documentais:** consultar `GET /rest/api/3/issue/{issueIdOrKey}/transitions?expand=transitions.fields` retorna transições do usuário para o estado atual. Falta de Transition issues produz lista vazia; leitura também depende de Browse Projects e segurança do item. O destino está em `to.id`; o identificador da operação está em `transition.id`. Metadados de campos incluem `required`, `schema`, `allowedValues` e operações aceitas. Executar `POST /rest/api/3/issue/{issueIdOrKey}/transitions` com `transition.id`; campos da tela podem ir em `fields`/`update`. Sucesso é `204`; há erros documentados `400`, `401`, `404`, `409`, `413` e `422`. Scopes clássicos recomendados: `read:jira-work` na consulta e `write:jira-work` na execução; os scopes granulares estão listados por operação. [Referência Issues, Get transitions e Transition issue](https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-issues/).

**Inferência:** arrastar para uma coluna exige resolver um status de destino e uma transição atualmente disponível para aquele item. ID de coluna, status e transição são conceitos distintos. Dois itens na mesma coluna podem oferecer operações diferentes. Metadados da tela não provam ausência de validações adicionais do workflow; o servidor continua sendo a autoridade no momento da escrita.

**Proposta:** consultar as transições ao iniciar a ação; filtrar destinos pelo mapeamento da coluna. Se houver múltiplas opções, solicitar escolha identificável em pt-BR; se faltar operação ou houver campo obrigatório não suportado, explicar a limitação e permitir abrir o item no Jira. Não preencher resolução, comentário ou outro campo por suposição. Após conflito ou recusa, recarregar estado e transições. Uma ação pendente não deve aparecer como concluída antes da confirmação.

### Ordenação não é transição

**Fato documental:** `PUT /rest/agile/1.0/issue/rank` altera posição antes/depois de outro item; aceita `rankBeforeIssue`, `rankAfterIssue`, `rankCustomFieldId`, até 50 itens e pode responder `207` com falhas parciais. Scope: `write:issue:jira-software`. [Referência Jira Software Issue](https://developer.atlassian.com/cloud/jira/software/rest/api-group-issue/).

**Proposta:** tratar mover de estado e reordenar como capacidades independentes. Não prometer que uma transição persiste a posição visual. Caso ranking entre no escopo, testar suas permissões e falhas separadamente; não executar dois writes ocultos como se fossem atômicos.

## Autenticação própria e plugin são contratos diferentes

**Fato documental:** a Atlassian recomenda OAuth 3LO para integrações e admite Basic auth com API token para scripts pessoais/bots. A autorização Basic respeita as permissões do usuário. [Security for other integrations](https://developer.atlassian.com/cloud/jira/platform/security-for-other-integrations/).

| Opção | Requisitos e limites comprovados |
| --- | --- |
| Token de API da própria conta | Basic auth usa e-mail e API token; senha de conta não é o contrato atual. Adequado documentalmente a chamadas pessoais e scripts. [Basic auth](https://developer.atlassian.com/cloud/jira/software/basic-auth-for-rest-apis/). |
| Token com scopes | A documentação oferece tokens limitados por scopes e usa a base `https://api.atlassian.com/ex/jira/{cloudId}`; tokens sem scopes usam a URL do site. Tokens têm validade escolhida entre 1 e 365 dias e revogação manual; não são permanentes. [Gerenciar API tokens](https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/). |
| OAuth 3LO próprio | Requer app no developer console, scopes configurados, consentimento, `state` imprevisível e callback registrado. Troca do código em `https://auth.atlassian.com/oauth/token` exige `client_id` **e `client_secret`**. A resposta informa `expires_in`. [Authorization Code Flow](https://developer.atlassian.com/cloud/oauth/getting-started/implementing-oauth-3lo/). |
| Renovação 3LO | `offline_access` permite refresh token; a troca exige `client_secret`, retorna um novo par e desabilita o refresh token usado. Erros de refresh podem exigir novo consentimento. [Refresh Token Flow](https://developer.atlassian.com/cloud/oauth/getting-started/refresh-tokens/). O prazo padrão de inatividade é 90 dias, com tolerância de reutilização de 10 minutos. [Guia Jira 3LO](https://developer.atlassian.com/cloud/jira/platform/oauth-2-3lo-apps/). |

**Fato documental:** `GET https://api.atlassian.com/oauth/token/accessible-resources`, autenticado com Bearer access token, fornece sites autorizados e `cloudId`; chamadas 3LO usam `https://api.atlassian.com/ex/jira/{cloudId}/{api}`. Não escolher o primeiro recurso como “site recém-autorizado”: a ordem não representa essa informação. Consentimento pode ser revogado ou substituído. [Making Calls to API](https://developer.atlassian.com/cloud/oauth/getting-started/making-calls-to-api/).

**Limite de evidência:** nos guias oficiais Jira Cloud 3LO e Authorization Code Flow consultados não foi encontrado contrato de cliente público com PKCE que dispense `client_secret`. Isso não é teste experimental de rejeição de PKCE nem prova de inexistência em todo produto Atlassian. Não se deve extrapolar documentação de Jira Data Center ou de MCP para a API REST Cloud. [Guia Jira 3LO](https://developer.atlassian.com/cloud/jira/platform/oauth-2-3lo-apps/), [Authorization Code Flow](https://developer.atlassian.com/cloud/oauth/getting-started/implementing-oauth-3lo/).

**Inferência:** o fluxo descrito tem requisitos de cliente confidencial. Embutir um segredo compartilhado num binário público não preserva confidencialidade; um listener local de callback apenas recebe o código, não elimina a exigência do segredo. Assim, “OAuth PKCE puramente desktop, sem servidor nem segredo” não está comprovado como opção implementável. Um app 3LO privado configurado pelo próprio autor é diferente de distribuir um cliente com segredo para terceiros; sua callback local ainda precisaria ser validada. Nenhuma dessas opções foi escolhida aqui.

**Fato documental de distribuição:** o guia Jira 3LO diz que apps são privados por padrão e podem habilitar compartilhamento. Também avisa que apps que coletam API tokens de clientes ou instruem clientes a criar apps 3LO individuais não cumprem suas exigências de apps Cloud. [Guia Jira 3LO](https://developer.atlassian.com/cloud/jira/platform/oauth-2-3lo-apps/). As exigências de apps publicam armazenamento seguro de segredos e proibição de coletar credenciais de usuários. [Security requirements for cloud apps](https://developer.atlassian.com/platform/marketplace/security-requirements/).

**Inferência:** a permissão documental para uso pessoal não deve ser apresentada como aval automático para transformar o repositório público em integração distribuída que coleta tokens. Uso pessoal e eventual distribuição exigem decisões explícitas distintas; esta pesquisa não fornece parecer jurídico.

**Fato documental sobre o plugin:** o Atlassian MCP é uma ponte hospedada em nuvem, com OAuth 2.1 ou API tokens e respeito às permissões existentes. [Atlassian MCP Server oficial](https://atlassian.github.io/atlassian-mcp-server/).

**Inferência:** a conexão já disponível no plugin demonstra o acesso daquela integração. Ela não entrega ao binário Tauri um token reutilizável, um registro OAuth próprio ou autorização implícita para extrair credenciais do plugin. Implementar acesso REST direto e consumir MCP seriam arquiteturas diferentes. Esta pesquisa não leu credenciais nem pressupõe qual mecanismo autentica a conexão existente.

**Limite financeiro:** a Atlassian oferece plano Cloud Free, sujeito a seus limites; isso não informa o plano da conta alvo. [Licenciamento Cloud](https://www.atlassian.com/licensing/cloud). Não foi encontrada aqui necessidade de comprar um servidor para chamadas REST pessoais com token; isto é inferência arquitetural, não garantia comercial de custo perpétuo zero. A pesquisa não verificou cobrança ou políticas específicas da organização, nem resolveu hospedagem segura de um cliente confidencial distribuído.

## Permissões, limites e recuperação

Scopes não substituem as permissões do usuário nem removem condições do workflow. A documentação recomenda levantar scopes por endpoint; não solicitar permissões administrativas globais por conveniência. [Scopes Jira](https://developer.atlassian.com/cloud/jira/platform/scopes-for-oauth-2-3LO-and-forge-apps/), [Security for other integrations](https://developer.atlassian.com/cloud/jira/platform/security-for-other-integrations/).

**Fatos documentais:** a referência atual distingue quota por pontos, burst e limite de escrita por item. A mudança de quotas de apps iniciada em 02/03/2026 abrange Forge, Connect e 3LO; a nota exclui tráfego de API token dessa mudança, mantendo burst. Respostas `429` indicam `RateLimit-Reason` e `Retry-After`; alguns `503` também podem trazer este último. A orientação é respeitar a espera, usar backoff exponencial com jitter e limitar tentativas; retry automático depende de idempotência. [Rate limiting](https://developer.atlassian.com/cloud/jira/platform/rate-limiting/).

**Limite documental:** a mesma página mistura conteúdo de rollout/beta e exemplos com taxas diferentes. Nenhuma taxa fixa foi assumida como orçamento garantido do monitor. Cabeçalhos e respostas do serviço devem orientar recuperação. [Rate limiting](https://developer.atlassian.com/cloud/jira/platform/rate-limiting/).

**Proposta:** polling moderado e suspenso durante backoff; leituras de configuração menos frequentes; relógio injetável e orçamento limitado de retries. Serializar ações por item. Se um POST tiver resultado desconhecido após queda de conexão, reler o item antes de propor nova execução, pois uma transição pode disparar efeitos adicionais. Distinguir autenticação inválida/expirada, acesso recusado, recurso indisponível, conflito e serviço temporariamente indisponível, sem imprimir corpos privados de erro. Desconectar apaga o segredo local; revogar no provedor é uma ação distinta, não uma consequência que se deva presumir.

## Evidências e fixtures futuras

Este arquivo é pesquisa, não implementação, ADR ou aprovação de autenticação. Foram lidos `AGENTS.md`, `docs/agents/sanitizacao.md`, a skill research, `GLOSSARY.md` e ADRs locais pertinentes. A spec v1 citada pelo repositório não está presente neste worktree; seu conteúdo não foi presumido. Nenhum build ou teste foi executado.

**Proposta para implementação posterior:** coletar respostas reais apenas quando autorizado e com projeção/sanitização em memória antes de imprimir ou gravar. Excluir auth, trocar consistentemente domínio, IDs, chaves, nomes, e-mails e conteúdo; preservar tipos, relações e paginação. Registrar origem, campos substituídos e variantes sintéticas separadamente. Fixtures desejadas:

- Seleção vazia/paginada e board `simple`; configuração real sanitizada com cinco colunas, inclusive relações de múltiplos status quando existirem.
- Itens em mais de uma página, status sem coluna e divergência entre board e projeto; não fabricar uma “captura real” para um caso não observado.
- Transição sem campos, com campos exigidos, destinos ambíguos, lista vazia por restrição, conflito e erro de validação.
- Token expirado/revogado, scopes insuficientes, falha de persistência segura, `429`/`Retry-After`, falha transitória e resultado de escrita desconhecido.
- Sentinelas sintéticas comprovando ausência de credenciais, títulos e conteúdo em logs, cache e notificações; snapshots UI a partir das mesmas projeções sanitizadas.

As futuras E/S devem passar pelas portas injetáveis do núcleo, incluindo HTTP, armazenamento seguro e relógio. O guardrail vigente exige GNOME Keyring e recusa de salvar sem ele; qualquer alteração para Windows pertence à decisão de domínio e persistência correspondente, não a este relatório.
