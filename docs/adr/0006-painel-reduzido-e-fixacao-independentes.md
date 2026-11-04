# Painel reduzido e fixação independentes

O contrato aprovado adota a composição **Núcleo de IA** para o Painel reduzido: uma coluna com IA, atenção de Agentes e resumos dos demais Módulos, mantendo o Relógio do Painel visível. No Windows, “Manter no topo” funciona nos modos completo e reduzido, preservado ao alternar; no Ubuntu/Wayland, a indisponibilidade é explícita, sem extensão adicional ou promessa de posicionamento que contradiga o ADR 0001.

O reduzido permanece aberto ao perder foco; o completo sem fixação fecha ao clicar fora. Com fixação ativa, ambos permanecem abertos até fechamento explícito, sem capturar o foco de outros aplicativos. Separar Modo e Fixação evita que reduzir imponha topmost ou que desfixar elimine o acompanhamento; ao custo de manter a janela visível enquanto o usuário trabalha, o reduzido passa a servir como acompanhamento contínuo.

Modo, fixação e tamanho de cada modo são lembrados entre execuções; no Windows, a posição também é restaurada e recuperada à área útil quando necessário. Primeiro uso completo/desfixado e autostart silencioso são preservados. O protótipo é evidência de discussão, não código de produção: resolução, origem e critérios de aceite estão em [Como deve funcionar a versão reduzida e a opção de manter no topo?](../../.scratch/jira-windows/issues/06-compact-panel.md).
