# Validação gráfica GPU Windows

Status: ready-for-human
Assignee: Codex / usuário
Parent: ../spec.md

## Critérios

- CI nativa Ubuntu/Windows e artefato portátil sem assinatura identificados.
- Rodar no Windows sem elevação, com Controle Inteligente preservado, após autorização da abertura pela interface.
- Conferir duas GPUs sem misturar uso/temperatura; comparar NVIDIA com a ferramenta do driver e explicar eventuais diferenças de amostragem.
- Conferir Sistema completo, resumo reduzido e relógio, falhas e retorno após ocultar/reabrir.
- Preservar perfil, cofre e portátil anterior; limpar ferramentas e cópias temporárias obsoletas.
- Registrar somente categorias/resultados, sem modelos, IDs, nomes privados ou capturas brutas no Git.

## Preparação concluída

CI [42499337778](https://github.com/lucasvss2/pc-monitor/actions/runs/42499337778) aprovada em Ubuntu e Windows, revisão `439393f`. Artefato portátil `windows-foundation-experimental-unsigned`, EXE SHA-256 `777F206DC011DF3BE9B78A0E5F35E460162CF3B0C07350EFBA261ED2A4802F96`, assinatura `NotSigned`. Digest do ZIP conferido com a API do GitHub. Teste silencioso: código zero, stdout/stderr vazios, processo comum sem elevação e Controle Inteligente `On`. Nenhum instalador foi aberto ou baixado no host para esta rodada.

Prévia do componente real com snapshot sintético conferida em completo 1100 × 700 e reduzido 420 × 620/360 × 480, sem rolagem horizontal e com relógio acessível. Servidor, arquivos de prévia, formatador temporário e artefatos de diagnóstico de formatação removidos. O binário candidato é preservado somente para a abertura autorizada; o portátil anterior continua intacto. A validação gráfica nativa aguarda a confirmação de execução exigida pela skill computer-use; não foi declarada concluída por teste silencioso ou prévia.
