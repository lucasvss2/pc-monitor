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

### Evidências nativas intermediárias — 04/11/2026

Na [execução do commit 55b22fd](https://github.com/lucasvss2/pc-monitor/actions/runs/41821611321), Ubuntu passou integralmente. No Windows: clippy passou; 240 testes do núcleo passaram, incluindo pipe nativo com reconexão, validação de destino COM, dispatch real por COM e callbacks sintéticos de energia. O teste separado de cofre real também passou: roundtrip Unicode, tipo genérico, `CRED_PERSIST_LOCAL_MACHINE`, exclusão e exclusão repetida; nenhum valor foi impresso. O teste de subprocesso passou com o executável Rust, PowerShell, caminho especial, stdin sintético e remoção de sentinelas de conteúdo antes do transporte.

Essa execução ainda falhou ao compilar o modo de fixtures no Windows por um import condicional de `mpsc`. A correção usa o caminho completo; clippy e testes de fixtures passam a ser passos separados para que uma falha do primeiro não seja ocultada por outro comando bem-sucedido no PowerShell. O build final e o artefato ainda aguardam confirmação.

Os testes usam entradas sintéticas e fixtures existentes sanitizadas, sem modificar configurações reais de agentes. A prova local adicional do lançador com um intérprete existente preservou 47 bytes de stdin sintético, incluindo CRLF, sem stderr; ela não substitui o teste do binário Rust executado na CI. `.gitattributes` preserva bytes de fixtures do núcleo e evita conversão de snapshots pelo checkout.

Após falhas internas no envio, a branch `codex/windows-native-validation` foi criada pela API com árvore idêntica à local, sem force-push ou remoção de commits. As branches de pesquisa/protótipo permaneceram locais. Os envios seguintes usam a autenticação já disponível no CLI, sem reconfiguração global do Git.

Pendências de aceite: sessão gráfica Windows 11 x64 para bandeja, foco, eventos reais das CLIs, banner e Central de Notificações, reinício e suspensão/retomada; regressão gráfica Ubuntu. [Roteiro de validação](../../../docs/windows-validacao.md). A descoberta de sessões Windows continua explicitamente parcial e o mínimo completo não é declarado implementado.
