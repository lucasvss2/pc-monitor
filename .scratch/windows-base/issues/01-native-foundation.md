# Base Windows, cofre, hooks e notificações

Status: ready-for-agent
Assignee: Codex
Parent: ../spec.md

## Escopo

Implementar a primeira etapa da [especificação](../../jira-windows/spec.md), com adaptadores nativos sem dependências Linux no build Windows. Não promover o incremento a suporte oficial antes do mínimo completo.

## Critérios de aceite

- Compilação e testes nativos Windows; CI preservando Ubuntu.
- Indicador: clique esquerdo abre/ativa, direito abre menu; fechar Painel preserva processo.
- Credential Manager real, usuário/máquina sem roaming, recusa segura e prova com credencial sintética removida após teste.
- Hooks opt-in, configuração aditiva/reversível, transporte local restrito ao usuário, subprocesso limitado e silencioso; não tocar configurações reais durante testes automatizados.
- Notificações com texto permitido, ativação pelo banner e Central abre Agentes sem alterar Âncora; nenhum conteúdo privado no payload persistido pelo Windows.
- Evidências distinguem testes automáticos, prova nativa e verificações interativas ainda pendentes; fixtures seguem sanitização.

## Comments

04/11/2026: implementação iniciada. SDK e ferramentas C++ detectados; Rust preparado em diretório temporário isolado, sem alterar PATH global. Nenhuma configuração real de agente será modificada automaticamente.

### Primeira validação

- UI: `npm run check`, 376 testes anteriores e build passaram; após acrescentar capacidade indisponível, check e os 53 testes do Painel passaram.
- Rust: formatação e metadados do manifesto passaram. O build local foi interrompido por Controle de Aplicativo do Windows, erro 4551 ao executar um build script de dependência. Não houve alteração de política nem tentativa de contornar o bloqueio.
- O usuário escolheu preparar a CI Windows e validá-la. Matriz proposta: Ubuntu e Windows nativos, com prova separada de cofre real usando credencial sintética e remoção ao terminar. Aceite gráfico de bandeja/notificação continua pendente.
- Revisão de publicação: caminhos alterados e material de planejamento revisados, sem credenciais/capturas reais. ICO inspecionado: contém exatamente o PNG público existente, sem conteúdo ou metadados adicionais. Identidades de commit devem usar noreply do mantenedor. Somente branch `codex/windows-*`; branches locais de pesquisa/protótipo não serão enviadas.
