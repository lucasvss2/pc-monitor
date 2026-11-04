# Quais fronteiras e evidências tornam a implementação pronta para começar?

Type: grilling
Labels: wayfinder:grilling
Status: resolved
Assignee: Codex / architecture_validation
Parent: ../map.md
Blocked by: 04, 05, 06, 07

Prerequisites: [Como escolher Trello ou Jira preservando estado e privacidade?](04-provider-contract.md); [Qual é o mínimo obrigatório do produto no Windows?](05-windows-minimum.md); [Como deve funcionar a versão reduzida e a opção de manter no topo?](06-compact-panel.md); [Qual horário o relógio do Painel deve apresentar?](07-system-clock.md)

## Question

Com os contratos de produto decididos, quais fronteiras de domínio/plataforma, migrações e evidências de teste e distribuição são suficientes para implementar sem novas decisões estruturais?

Decidir separação de dependências/adaptadores por alvo preservando E/S injetável, contratos compartilhados de snapshot/operação/capacidade, proteção contra regressão no Ubuntu, cobertura de privacidade, fixtures reais sanitizadas versus variantes sintéticas, testes nativos Windows/Linux, instalador e requisitos de distribuição de custo zero. Definir critérios de aceite, ordem de adoção e forma da especificação consolidada; não executar build/portabilidade como parte deste ticket de decisão.

## Context

Os contratos de produto e a composição visual estão aprovados nos pré-requisitos. Ao modelar a janela, explicitar Modo/Fixação/geometrias lembradas e a diferença entre mudança de Modo escolhida pelo usuário e apresentação contextual para Configurações, detalhes de Módulo ou notificação; preservar retorno e Âncora conforme seus contratos. Definir limites nativos de tamanho/área útil a partir de medição e dos critérios de legibilidade aprovados, sem promover o código descartável do protótipo para produção.

## Comments

### Auditoria local — 04/11/2026

Auditoria somente leitura, sem rede, host, credenciais, alterações, builds ou testes:

- A fronteira de E/S já é formada por portas distribuídas (`src-tauri/src/ports.rs`, `persistence.rs`, `lifecycle.rs` e portas dos Módulos). Um único seam não significa um trait único. Concretos Linux e factories ainda se misturam aos contratos e aos `start` dos coletores; precisam de composição por alvo, inclusive Cargo/features e captura GTK.
- `src/lib/Panel.svelte:54` aceita snapshots por `updatedAt`; limpar leitura com horário nulo ou retroceder hora pode impedir a atualização correta. Revisão de publicação e geração de contexto precisam ser distintas do instante civil e da geração de conectividade/suspensão atual.
- `src-tauri/src/state.rs:81` descarta o conjunto de preferências em erro de desserialização. Migrar a identidade Trello e defaults de novos campos de forma localizada é necessário para preservar o restante.
- AppState já vive fora da webview. Configurações/retorno de foco hoje estão na UI, e a janela é destruída ao fechar; preferências de janela e Apresentação contextual não podem depender apenas da instância da UI.
- `Clock.now` é civil e também atende alguns prazos; medir duração/timeout/backoff por fonte monotônica e reavaliar após retomada evita usar saltos de hora como duração. CI e release atuais ainda são apenas Ubuntu/deb/AppImage.

Os achados sustentam contratos técnicos; não reabrem os requisitos de produto já aprovados nem comprovam execução Windows.

### Decisões confirmadas — 04/11/2026

O usuário aceitou as três recomendações:

- Um núcleo Rust e uma UI compartilhados, com adaptadores Linux/Windows e Trello/Jira, preservando as portas injetáveis como fronteira uniforme de E/S.
- Configurações, detalhes de Módulo e notificações podem apresentar o completo temporariamente a partir do reduzido, com retorno, sem substituir o Modo preferido salvo. Notificações preservam a Âncora.
- Começar pela base Windows e pelas evidências de maior risco (cofre, hooks, notificações); depois Tarefas/Jira, restante do mínimo Windows e Painel reduzido/relógio, preservando Ubuntu. Publicação depende do mínimo completo aprovado.

Termos registrados no [GLOSSARY.md](../../../GLOSSARY.md). A especificação técnica foi consolidada na resolução abaixo; seus critérios são requisitos futuros, não resultados de testes já executados.

## Answer

Decisão confirmada pelo usuário em 04/11/2026. O ativo desta resolução é a [especificação de Tarefas, Windows e apresentação do Painel](../spec.md), que integra os contratos aprovados sem duplicar as respostas dos demais tickets.

A implementação terá núcleo Rust e UI compartilhados, adaptadores nativos por alvo e adaptadores Trello/Jira nas portas de E/S. A especificação define publicação por revisão e geração de contexto, operações com resultado não confirmado, limpeza durável recuperável, migração localizada de preferências, separação de tempo civil/duração e fronteiras para cofre, hooks e notificações. O [ADR de núcleo compartilhado](../../../docs/adr/0007-nucleo-compartilhado-e-adaptadores-nativos.md) registra a escolha durável e seus custos.

Modo preferido e Apresentação contextual estão separados: detalhes, Configurações e notificações podem abrir o completo com retorno sem substituir a preferência. A sequência aprovada começa pela base Windows e pelos riscos nativos; segue por Tarefas/Jira, restante do mínimo Windows e Painel/relógio. Publicação exige o mínimo completo e evidências nativas nos dois alvos, além dos testes nas portas e snapshots.

Não restou decisão estrutural aberta para iniciar a implementação. Medidas exatas de janela e provas de bindings/serviços são trabalho verificável da implementação, com critérios definidos; não estão declaradas concluídas. Limitação que exija mudar requisito aprovado deve gerar nova decisão, em vez de reduzir silenciosamente o mínimo.

Validação desta resolução: respostas explícitas às três escolhas, auditoria local somente leitura, revisão dos contratos e referências de documentação. Nenhum código de produção, credencial ou configuração do host foi alterado nesta etapa; builds, testes funcionais e aceite nativo continuam pendentes para execução futura.

Conferência final: 24 arquivos Markdown verificados, vínculos locais válidos, dependências sem ciclos, nove decisões resolvidas e nenhuma restante. A revisão de espaços e `git diff --check` passaram. Os textos novos usam apenas contratos e referências públicas/sanitizadas, sem capturas do host ou valores privados. Não foram executados testes da aplicação para esta alteração documental.
