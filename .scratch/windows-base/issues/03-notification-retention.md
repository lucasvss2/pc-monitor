# Notificação Codex removida pela ausência de consulta periódica

Status: ready-for-agent
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
