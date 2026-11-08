# Documentar adaptações para Windows

Status: resolved
Assignee: Codex
Parent: ../spec.md

Solicitação adicional do usuário em 08/11/2026.

## Critérios

- Explicar uso do portátil, pasta estável, WebView2 e diferença para o instalador bloqueado por política.
- Explicar ausência de exigência de administrador e de assinatura própria, sem prometer execução de toda build com SAC ativo.
- Listar fontes Windows de CPU/RAM/discos/bateria/rede e as novas fontes GPU.
- Explicar uso por motor, associação entre GPUs, temperatura NVIDIA e limitação CPU AMD comprovada.
- Descrever cadência, suspensão/retomada, estados sem leitura e limites por hardware/driver.
- Separar recursos implementados, provas automatizadas, prova gráfica e aceites pendentes.
- Atualizar guias de uso/desenvolvimento e decisões pertinentes com links para os roteiros.

## Resultado

Criado `docs/windows.md`, vinculado no README e no guia de desenvolvimento. O guia cobre distribuição, permissões, fontes por Módulo, semântica/múltiplas GPUs, sensor CPU AMD não suportado neste driver, ciclo de coleta, privacidade e separação de evidências/aceites. ADR 0005 e glossário atualizados. A documentação acompanha a implementação; não encerra a validação nativa do ticket 04.
