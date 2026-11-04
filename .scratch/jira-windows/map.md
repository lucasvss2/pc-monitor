# Jira, Windows e apresentação reduzida do Painel

Labels: wayfinder:map
Status: resolved

## Destination

Chegar a uma especificação com decisões e critérios de aceite suficientes para escolher Trello ou Jira nas Configurações, oferecer o app nativo no Windows mantendo Ubuntu e acrescentar versão reduzida do Painel, opção de manter no topo e relógio acompanhando o Windows.
O mapa termina quando essas decisões estiverem resolvidas; a implementação é uma etapa posterior.

Destino alcançado em 04/11/2026: [especificação para implementação](spec.md). Todos os tickets de decisão foram resolvidos; suporte nativo e critérios de aceite ainda precisam ser implementados e verificados.

## Notes

- Escopo confirmado pelo usuário em 03/11/2026: Trello ou Jira, um provedor ativo; visualizar itens e mudar estado; Windows nativo com Ubuntu preservado; mínimo obrigatório por plataforma com diferenças explícitas; cofre nativo seguro por sistema e recusa de salvar sem cofre, sem fallback em arquivo.
- A solicitação seguinte acrescentou botão para versão reduzida, manter no topo e relógio sincronizado com o Windows. Os contratos aprovados e a evidência visual vivem nos tickets indicados em Decisions so far.
- Fonte Jira inspecionada via conexão Atlassian: board encontrado a partir de referências ao projeto indicado pelo usuário, hospedado no Jira Cloud, tipo retornado `simple`, cinco colunas. O nome do board não foi inferido do nome do repositório. Nomes, chaves, IDs, hostname, URLs privadas e conteúdo dos itens foram omitidos. Essa inspeção não valida a autenticação futura do app nem a disponibilidade de transições por item.
- Usar wayfinder, grilling e domain-modeling em cada sessão de decisões; research nas pesquisas; prototype na discussão visual. Ler `GLOSSARY.md`, ADRs pertinentes e `docs/agents/sanitizacao.md`.
- Tracker: `docs/agents/issue-tracker.md`. Tickets são filhos deste mapa; `Blocked by` expressa dependências. A fronteira contém filhos abertos, não reivindicados e sem bloqueadores abertos, em ordem numérica.
- A spec v1 referenciada em `.scratch/kvojps-monitor/spec.md` não está nesta árvore. Usar implementação, README, `docs/desenvolvimento.md` e ADRs como evidências disponíveis, sem reconstruir a spec ausente como se fosse original.
- Preservar custo zero, operação local, ausência de telemetria/servidor próprio, E/S injetável e fixtures sanitizadas. Conteúdo sensível não entra em logs, cache ou notificações. Hooks/status line exigem clique explícito, instalação aditiva e reversível. Token Claude só em memória, nunca renovado.
- ADR 0001 trata do GNOME e não define experiência Windows; ADR 0002 continua aplicável ao Claude; ADR 0003 mantém coleta de tráfego sem privilégio. Não assumir que nomes de coluna Jira equivalem a transições.
- Pesquisas usam apenas fontes primárias públicas e fatos sanitizados do repo. Branches `research/*` são locais: nunca fazer push nem abrir PR para merge dessas pesquisas, conforme AGENTS.md.
- Termos novos, mudanças de domínio e ADRs serão registrados quando as respectivas decisões forem resolvidas; o escopo aceito aqui não afirma que o código já suporta essas capacidades.

## Decisions so far

<!-- Somente índice de tickets resolvidos; detalhes e resposta vivem no ticket. -->

- Revisão durante a implementação em 04/11/2026: [O Painel deve permanecer aberto ao perder foco?](issues/10-panel-explicit-close.md). O usuário escolheu manter sempre aberto e fechar pelo X ou Escape, substituindo o fechamento automático do completo desfixado. Especificação, matriz e ADR 0006 foram atualizados; a nova regra exige validação nativa.

- [Quais contratos do Jira Cloud atendem ao board alvo?](issues/01-jira-cloud-api.md): APIs públicas permitem leitura/transições; modalidade do board e autenticação própria exigem contratos explícitos.
- [Quais capacidades nativas do Windows substituem os adaptadores Linux?](issues/02-windows-platform.md): há candidatos nativos, com limites de métricas, cofre local e comportamento de janela que precisam entrar no contrato.
- [Como acompanhar IA e Sessões de agente no Windows?](issues/03-windows-agents.md): execução nativa e WSL têm contratos distintos; os adaptadores atuais não comprovam cobertura global de sessões Windows.
- [Como escolher Trello ou Jira preservando estado e privacidade?](issues/04-provider-contract.md): Tarefas terá um provedor ativo, contexto preservado no cofre e leitura Jira apenas em memória, com troca e escritas isoladas.
- [Quando a assinatura do app Windows é necessária?](issues/09-windows-signing.md): EXE direto pode dispensar assinatura própria; políticas podem bloquear e notificações exigem ativação validada separadamente.
- [Qual é o mínimo obrigatório do produto no Windows?](issues/05-windows-minimum.md): Windows 11 x64 com CLIs nativos, Módulos básicos, cofre local e EXE por usuário inicialmente sem assinatura.
- [Qual horário o relógio do Painel deve apresentar?](issues/07-system-clock.md): hora/fuso locais do sistema, formato pt-BR de 24 horas com segundos e data, visível nos dois modos do Painel.
- [Como deve funcionar a versão reduzida e a opção de manter no topo?](issues/06-compact-panel.md): Núcleo de IA aprovado; modo/fixação independentes, reduzido persistente sem foco e preferências de janela lembradas.
- [Quais fronteiras e evidências tornam a implementação pronta para começar?](issues/08-architecture-validation.md): núcleo/UI compartilhados, adaptadores por alvo/provedor, expansão contextual sem mudar preferência e base Windows com riscos nativos primeiro.

## Not yet specified

- Nenhuma decisão em aberto após a consolidação. Uma limitação descoberta na implementação que exija mudar requisito aprovado deve originar uma nova decisão.

## Out of scope

- Implementar, empacotar ou publicar a nova versão durante este mapa.
- Exibir Trello e Jira simultaneamente ou remover a integração Trello existente.
- Criar/editar conteúdo de itens, comentários ou administrar sprints/backlog; o recorte inicial é visualizar e mudar estado.
- Servidor próprio, telemetria e serviços privilegiados para obter paridade de métricas.
- Ajustar o relógio do sistema operacional ou introduzir sincronização NTP própria sem nova decisão explícita; o pedido atual é exibir o horário acompanhando o Windows.
- Generalizar a integração para Jira Data Center antes de uma necessidade demonstrada; o serviço verificado é Jira Cloud.
- Onboarding Jira multiusuário, OAuth para distribuição e formulários de edição de itens não integram o contrato pessoal inicial de [Como escolher Trello ou Jira preservando estado e privacidade?](issues/04-provider-contract.md).
- Windows 10/ARM64/x86, agentes no WSL, sessões exclusivas de app/IDE, tarefas persistidas sem processo e foco obrigatório de terminal externo não integram o mínimo inicial de [Qual é o mínimo obrigatório do produto no Windows?](issues/05-windows-minimum.md).
- Compra/inscrição em serviço de assinatura, publicação Store/MSIX e garantia de execução sob toda política de segurança ficam fora da rota inicial sem assinatura; alternativas e limites estão em [Quando a assinatura do app Windows é necessária?](issues/09-windows-signing.md).
