# Notificação Codex removida pela ausência de consulta periódica

Status: ready-for-human
Assignee: Codex
Parent: ../spec.md

## Reprodução nativa

No Windows 11 com o EXE de `1d38623`, hooks ativados por clique do usuário e notificador WinRT habilitado, um evento de atenção inteiramente sintético executou em 421 ms, silenciosamente. O usuário não viu banner. A consulta inicial ao histórico exclusivo do monitor encontrou zero notificações.

Um probe privado iniciou a observação do histórico antes de enviar outro evento sintético pelo comando registrado e verificado do próprio monitor. Resultado: máximo de uma notificação, primeira leitura aos 421 ms, remoção aos 1.452 ms e zero ao final de quatro segundos; nenhuma sentinela privada no XML. O comando terminou com `FAIL: waiting notification disappears from monitor history`. Não foram lidas nem capturadas notificações de outros aplicativos.

Hipóteses comparadas: remoção prematura na reconciliação, falha de envio nativo, ou supressão de banner com permanência na Central. O histórico transitório confirma envio e remoção; não prova apresentação visual do banner nem elimina uma supressão adicional pelo sistema.

## Causa em avaliação

A composição Windows ainda não tem adaptador de polling Codex. `Observation.codex = None` não fornece evidência de encerramento, mas a reconciliação usa apenas a lista Claude, remove a sessão Codex criada pelo hook e fecha sua notificação no próximo ciclo. Uma lista Claude vazia autoritativa não responde pela sessão do outro provedor.

## Critérios

- Teste no núcleo reproduz hook Codex + leitura Claude vazia + ausência de adaptador Codex, mantendo leitura indisponível explícita e last_success_at anterior.
- Ausência de leitura não encerra sessão; SessionEnd explícito ainda encerra e remove a notificação. A confirmação periódica real continua autoritativa quando o adaptador existe.
- A retenção em memória do último hook não comprova processo vivo nem cobertura completa; liveness/descoberta Windows permanece requisito posterior do mínimo e não pode ser declarada implementada aqui.
- Repetir probe nativo com notificação ainda presente após quatro segundos, além de banner/Central e preservação da Âncora.
- Desconectar os hooks por clique do usuário, conferir reversão e remover temporários/entradas de teste ao encerrar.

## Comments

Teste de regressão acrescentado antes da correção; execução pela CI, pois a instalação temporária Rust foi removida na limpeza anterior. Nenhuma política de segurança foi alterada. A consulta inicialmente gravada como script PowerShell foi recusada pela política de arquivos; as leituras seguintes usaram comandos permitidos sem alterar a política nem usar ExecutionPolicy Bypass.

### Vermelho no núcleo e correção

A [CI de dd811c9](https://github.com/lucasvss2/pc-monitor/actions/runs/41848146566) confirmou a falha funcional nos dois SOs: `codex_hook_without_polling_is_not_closed_by_an_unrelated_claude_read` encontrou zero sessões quando esperava uma. Os demais testes do núcleo passaram. As rodadas anteriores pararam em formatação e não foram usadas como evidência funcional.

A ausência de adaptador passa a reutilizar o tratamento de fonte indisponível quando há observação Codex anterior: conserva o último estado em memória, marca falha de consulta e mantém last_success_at anterior. Uma resposta válida do adaptador continua autoritativa, e SessionEnd explícito continua removendo sessão e notificação. O teste também verifica que a leitura Claude encerra sua própria sessão sem encerrar a do outro provedor.

### Resultado nativo e reversão

A CI de dce742a passou nos dois SOs. O probe nativo encontrou uma nova tag, mantida ao final de quatro segundos, sem sentinelas de conteúdo. O último EXE cffd77f preserva a correção e passou a regressão na CI Ubuntu/Windows. Na sessão gráfica, banner e Central abriram Agentes; ambos preservaram a Âncora controlada. A prova não acrescenta descoberta de processos vivos nem consulta periódica Codex completa.

Ao terminar os testes de hooks, o usuário desligou Detecção instantânea. O manifesto foi removido; o arquivo preexistente corresponde ao hash semântico inicial e o arquivo originalmente ausente voltou a ficar ausente. Só diretórios criados pelo teste e vazios foram removidos. Artefatos substituídos foram apagados. A limpeza final do EXE e da identidade foi concluída no fechamento desta sessão, registrada no ticket 05.
