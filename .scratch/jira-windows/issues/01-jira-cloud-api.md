# Quais contratos do Jira Cloud atendem ao board alvo?

Type: research
Labels: wayfinder:research
Status: resolved
Assignee: Codex / jira_api_research
Research branch: research/jira-windows-jira-api (local; sem push)
Research artifact: ../research/jira-cloud.md
Parent: ../map.md
Blocked by:

## Question

Quais APIs e opções de autenticação permitem a um app pessoal local, sem servidor próprio e de custo zero, selecionar e ler o board Jira Cloud alvo e mudar o estado de seus itens respeitando permissões, transições e campos obrigatórios?

O board inspecionado retorna tipo `simple` e cinco colunas; nenhuma identidade privada deve ser reproduzida. Investigar seleção de board/contexto, mapeamento coluna/status, transições por item, busca/paginação, ordenação/ranking, campos mínimos, limites/backoff, expiração/revogação e opções de credenciais. Distinguir o acesso do plugin Atlassian da autenticação própria do monitor. Não escolher uma opção de produto pelo usuário nem acessar itens privados durante a pesquisa pública.

## Context

- Código atual: `src-tauri/src/trello/api.rs`, `src-tauri/src/trello/mod.rs`, `src-tauri/src/persistence.rs`, `src/lib/trello/TrelloModule.svelte`.
- Publicar um relatório sanitizado com fontes primárias e limites da evidência. Nenhuma mutação real no Jira é necessária.

## Answer

Pesquisa documental concluída em 03/11/2026. O Jira Cloud oferece contratos públicos para seleção/leitura de board e transições por item; `simple` não distingue Scrum de Kanban. Coluna, status e transição são entidades distintas, e campos obrigatórios e permissões podem impedir uma ação. APIs enhanced e paginação por token devem informar a integração nova.

A autenticação do monitor é uma decisão ainda aberta: token pessoal e OAuth 3LO têm requisitos e limites diferentes; os fluxos 3LO documentados exigem `client_secret`, e PKCE sem segredo não foi comprovado. A conexão do plugin não fornece autenticação reutilizável pelo binário. A decisão seguinte deve distinguir uso pessoal e distribuição.

Evidência e contexto para retomada: [Jira Cloud: contratos para o Módulo local](../research/jira-cloud.md). Relatório público, com fatos/inferências/propostas separados; sem captura de credenciais ou conteúdo privado e sem mutações no serviço. Validação documental e revisão textual; nenhum teste de integração real, build ou implementação realizado.
