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

Prévia do componente real com snapshot sintético conferida em completo 1100 × 700 e reduzido 420 × 620/360 × 480, sem rolagem horizontal e com relógio acessível. Servidor, arquivos de prévia, formatador temporário e artefatos de diagnóstico de formatação removidos. Nessa etapa o binário candidato foi preservado para a abertura autorizada, mantendo o portátil anterior intacto; a prova nativa posterior está registrada abaixo, separada do teste silencioso e da prévia.

## Prova nativa — 08/11/2026

O usuário autorizou a substituição da instância e a abertura da candidata. O EXE correto foi identificado por hash e caminho, com uma única instância. O token do processo do monitor foi consultado somente quanto à elevação: não elevado. O Controle Inteligente permaneceu `On`; nenhum componente adicional ou mudança de política foi necessário.

O reduzido mostrou uso separado de NVIDIA e AMD, temperatura NVIDIA e relógio ampliado. O Sistema completo mostrou NVIDIA com uso/temperatura e AMD com uso/temperatura indisponível; CPU/RAM/disco permaneceram legíveis. A temperatura exibida coincidiu com a ferramenta NVIDIA já instalada dentro da tolerância de 3 °C para amostragem/arredondamento, sem tratar a definição de utilização NVML como equivalente à de motor PDH. Ao voltar ao reduzido, o estado inicial sem percentual deu lugar a nova amostra válida. X fechou a janela mantendo o processo; a reabertura retomou os valores.

A Âncora inicial foi restaurada pela UI após a navegação de teste. Modo, fixação, aparência e preferências de notificações foram comparados e preservados. A cópia foi promovida para `%LOCALAPPDATA%/Programs/kvojps-monitor-portable/439393f/`, reaberta e conferida novamente; Jira retomou sua conexão sem novo token. O hash do portátil anterior permaneceu igual. A pasta temporária do teste, ZIP, EXE duplicado e referências privadas das preferências foram removidos após conferir ausência de processos/ativação apontando para ela. A cópia estável em uso e o portátil anterior foram preservados.

Não foram versionadas capturas reais ou métricas/identificadores do perfil. O aceite visual final foi apresentado ao usuário e permanece pendente de sua resposta; os testes físicos de suspensão pertencem à pendência global já registrada, com retomada coberta neste incremento pelas portas injetadas.
