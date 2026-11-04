# Base Windows, cofre, hooks e notificações

Status: ready-for-human
Assignee: maintainer
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

### Validação do executável e estabilização da prova Linux — 04/11/2026

A [rodada 7cb78cd](https://github.com/lucasvss2/pc-monitor/actions/runs/41828895096/job/126570641086) passou integralmente no Windows, incluindo o teste do hook no EXE de release GUI. O artefato temporário dessa rodada foi inspecionado sem execução local: PE x64, subsistema GUI, tabela de certificados ausente, 9.512.960 bytes e SHA-256 `1ed90811d01b9d3bedb40031ce3724ac3cfb1ebde9cabe6bd9895096d0383d44`. Essa inspeção comprova o formato e a ausência de assinatura embutida, sem garantir execução sob políticas de segurança.

Na mesma rodada e numa repetição anterior, os testes Linux de tempo total de subprocesso falharam de forma intermitente. Serializar as três inicializações resolveu parte das falhas, mas a primeira carga GUI ainda consumiu cerca de 264 ms com stdin travado. A prova foi separada: a leitura nativa conserva 100 ms e tem teste direto com descritor local aberto, verificado abaixo de 200 ms; o processo completo segue o orçamento externo Linux já existente de um segundo. O teste mata/recolhe o filho ao exceder esse orçamento. Nenhum timeout de produção Linux foi ampliado, nem foram retiradas as verificações de silêncio e conteúdo.

O [job Ubuntu de 0e56529](https://github.com/lucasvss2/pc-monitor/actions/runs/41831439481/job/126579341890) passou: 381 testes de UI, 307 testes do núcleo, três testes de subprocesso, 15 testes de fixtures, fmt, clippy e build Tauri. As evidências não incluem eventos reais de CLIs nem uma sessão gráfica.

### Resultado do incremento — pronto para aceite manual

A [CI de 0e56529](https://github.com/lucasvss2/pc-monitor/actions/runs/41831439481) passou integralmente em Ubuntu 24.04 e no runner Windows 2025. Windows: 381 testes de UI, 240 do núcleo, prova separada do cofre real, teste de hook debug, 12 testes de fixtures, fmt/clippy, build e repetição do hook no EXE GUI de release. Ubuntu: os resultados completos estão acima. A prova COM verifica dispatch e destino; não comprova entrega nem clique real pelo banner ou Central.

[Artefato experimental sem assinatura](https://github.com/lucasvss2/pc-monitor/actions/runs/41831439481/artifacts/13323004402), temporário até 11/11/2026: PE x64/GUI, tabela de certificados ausente, 9.512.960 bytes; SHA-256 do EXE `db47c203ad057bbec236cb66522c63259f525a9140402db70acdd83c567c0768`. Inspecionado sem execução local. A CI usou a fonte `0e56529238558d4c965d59db5e8f4bc05d5541a9`; o commit seguinte registra apenas esta evidência e dispensa nova rodada de build.

O ticket passa a `ready-for-human` exclusivamente para os cenários gráficos do roteiro, eventos reais das CLIs e regressão gráfica Ubuntu ainda pendentes. Não representa aceite total do incremento nem suporte oficial Windows. A implementação posterior de descoberta/status line, demais Módulos, Jira, instalação e Painel/relógio conserva o escopo e a ordem da especificação aprovada.

Revisão de publicação final: diffs do incremento e evidências em `.scratch/`/`docs/` revisados; apenas fixtures existentes sanitizadas e entradas sintéticas, sem configurações pessoais ou conteúdo real de sessões. O artefato ficou fora da árvore Git; somente a branch `codex/windows-native-validation` recebeu estes commits, com identidade noreply. Nenhuma política de segurança, configuração global Git ou branch de pesquisa/protótipo foi alterada para concluir a validação.

### Início do aceite gráfico — 04/11/2026

Indicador visível confirmado pelo usuário, mas o Painel não abriu no clique esquerdo nem na segunda execução. Aceite interrompido antes dos cenários dependentes. Diagnóstico em [Painel Windows não abre pelo Indicador](02-panel-opening.md); os resultados automáticos anteriores continuam válidos, sem substituir este cenário reprovado.

Após a correção de foco em `a055512`, o Painel abriu e sua interface foi inspecionada no Windows 11. A sessão de Computer Use foi interrompida ao ativar Firefox por falta de suporte à aplicação de política de URLs nesse navegador. Restante do aceite gráfico pendente; detalhes e limpeza no [ticket do Painel](02-panel-opening.md).

### Continuação do aceite gráfico e controle de fechar — 04/11/2026

A [CI sem instrumentação, fe1d4f3](https://github.com/lucasvss2/pc-monitor/actions/runs/41840977069), passou nos dois SOs. No Windows 11, segunda abertura exibiu um Painel mantendo uma única instância; ativar a Calculadora fechou o Painel e preservou o processo. O usuário confirmou clique esquerdo abrindo o Painel e clique direito mostrando o menu do Indicador.

O usuário apontou a ausência de um botão visível de fechar e pediu esse controle. O cabeçalho passa a exibir X com nome acessível “Fechar Painel” e dica indicando que o Indicador continua ativo. Ele usa o mesmo comando existente de Escape/fechamento nativo, nos Módulos e nas Configurações. A opção Sair continua no menu do Indicador; o contrato de fechar o Painel é preservado.

Antes de testar hooks, foram registrados apenas hashes semânticos privados da configuração anterior para comparação posterior; nenhuma configuração de agente foi alterada nesta fase. Codex CLI nativo 0.154.0 encontrado; Claude Code não encontrado, portanto seu cenário real não está coberto nesta máquina.

### Resultado da continuação gráfica — 04/11/2026

A [CI de cffd77f](https://github.com/lucasvss2/pc-monitor/actions/runs/41867458322) passou integralmente em Ubuntu e Windows. A sessão Windows confirmou o botão X preservando o Indicador, clique esquerdo/direito, foco, ligar/desligar início automático e relançamento silencioso. A falha de remoção do início automático foi reproduzida primeiro em teste nativo isolado e corrigida; resultado e limite de logon estão no [ticket 04](04-autostart-removal.md).

A notificação removida prematuramente foi corrigida no núcleo, com teste vermelho/verde e probe nativo; a retenção não comprova liveness. A ativação agora usa protocolo fixo, com argumentos restritos e associação por usuário que preserva outro proprietário. O usuário confirmou banner abrindo Agentes e, em novo alerta com processo previamente encerrado, Central reabrindo Agentes. Em ambos os casos, a Âncora controlada diferente foi preservada; uma única instância foi conferida. A abertura para essa prova foi feita normalmente pelo menu Iniciar, com atalho temporário de identidade verificado; os probes iniciados pelas ferramentas tiveram resultados de registro divergentes. Detalhes e limitações nos [tickets 03](03-notification-retention.md) e [05](05-notification-cold-activation.md).

Hooks foram ativados/reconectados/desligados apenas por cliques explícitos. Ao concluir, a configuração preexistente corresponde ao hash semântico anterior e o arquivo originalmente ausente foi removido, assim como manifesto e novos diretórios vazios. Alertas, atalhos, associações, processos e artefatos temporários do teste foram limpos; Âncora restaurada e início automático desligado. Nenhum Firefox foi usado nesta continuação, e não se alteraram proteções do Windows.

Por escolha do usuário, reconexão e suspensão/retomada ficam pendentes. Também permanecem separados: logon real, eventos reais das CLIs, políticas de notificações/Não Incomodar, regressão gráfica Ubuntu e identidade/ativação/limpeza do futuro instalador por usuário. O incremento continua ready-for-human e não declara mínimo Windows completo, sessões vivas plenamente descobertas ou suporte oficial.
