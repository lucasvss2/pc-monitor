# Quais capacidades nativas do Windows substituem os adaptadores Linux?

Type: research
Labels: wayfinder:research
Status: resolved
Assignee: Codex / windows_platform_research
Research branch: research/jira-windows-platform (local; sem push)
Research artifact: ../research/windows-platform.md
Parent: ../map.md
Blocked by:

## Question

Quais capacidades oficiais do Tauri e do Windows permitem portar o Indicador, Painel, cofre, notificações, tema, suspensão, autostart, métricas de Rede/Sistema e distribuição sem privilégios elevados, mantendo os contratos de privacidade?

Produzir matriz de adaptadores atuais versus alternativas verificadas, dependências por alvo, limitações e verificações nativas necessárias. Cobrir Credential Manager/cofre e falha fechada; abertura de links; mínimo de SO/WebView2/arquitetura e CI/instalador; versão reduzida, always-on-top e exibição de hora/fuso do sistema após suspensão ou ajustes. Não prometer sensores/tráfego por processo sem evidência nem escolher mínimo de produto.

## Context

- `src-tauri/Cargo.toml`, `src-tauri/src/ports.rs`, `indicator.rs`, `panel.rs`, `lifecycle.rs`, `autostart.rs`, `network/`, `system/`, `desktop_theme.rs`, `.github/workflows/`.
- ADRs 0001 e 0003. O usuário aceitou cofre nativo por sistema e diferenças de capacidade explícitas, mas não definiu o mínimo Windows.

## Answer

Pesquisa documental concluída em 03/11/2026. Tauri e APIs Windows oferecem candidatos para desktop e métricas básicas, mas as dependências e adaptadores Linux atuais precisam ser isolados por alvo. Tráfego por processo e sensores não têm paridade sem privilégio comprovada. Notificações com ativação, autostart, suspensão, instalador e cofre requerem experimentos nativos antes de afirmar suporte.

O keyring v3 pode selecionar backend mock sem feature aplicável ao alvo; seu backend Windows usa persistência potencialmente itinerante. Escolher cofre deve assegurar backend real, persistência local e falha fechada. Manter no topo tem caminho Windows; a API de janela no Wayland não oferece a mesma garantia. O relógio pode acompanhar hora/fuso do sistema, relendo após ajustes e retomada, sem NTP próprio; isso não define ainda formato nem interação do Painel.

Evidência e contexto: [Plataforma Windows nativa: capacidades, limites e verificações](../research/windows-platform.md). Relatório revisado com matriz e fontes oficiais; nenhum build, instalação, coleta de dados do host ou teste nativo foi executado. Mínimo de SO, backend, experiência e distribuição permanecem decisões abertas.
