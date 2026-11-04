# Configurações, quadro e validação

Status: ready-for-human
Assignee: Codex
Parent: ../spec.md

Seletor de fonte e conexão específica no Módulo Tarefas; quadro/transições/limitações em pt-BR. Testes nas portas e UI para migração, falhas/reinício, fontes isoladas, resultado incerto, respostas tardias e privacidade. CI Ubuntu/Windows; leitura/ação reais com conta de teste autorizada são aceite separado, sem editar itens reais sem autorização.

## Comments

Interface e guia implementados: fonte identificada, formulários próprios, transições explícitas, navegador para campos obrigatórios, limpeza recuperável e descarte do estado transitório ao mudar de contexto. Um teste reproduziu a mensagem incorreta de desconexão no reinício offline; a correção distingue pausa/ausência de leitura de conexão removida. A UI passou 391 testes na CI Windows; checagem Svelte sem erros/avisos e build local aprovados. A fixture tasks=full-jira permite apresentar o quadro sem rede/cofre. Aceite gráfico e de autenticação/ação com conta de teste permanece pendente nos dois sistemas, conforme docs/tarefas-validacao.md.

A CI final 41895134794 passou integralmente nos dois runners na revisão 5269be6. Ubuntu: 333 testes unitários, três de integração e 16 de fixtures; Windows: 268 unitários, integração de hooks e cofre sintético separado, 13 de fixtures; ambos geraram executáveis de produção. Limites da autenticação pessoal documentados: token sem escopos no site, sem suporte ao gateway de tokens com escopos. Temporários locais e artefato intermediário obsoleto removidos.
