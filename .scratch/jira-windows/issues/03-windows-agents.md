# Como acompanhar IA e Sessões de agente no Windows?

Type: research
Labels: wayfinder:research
Status: resolved
Assignee: Codex / windows_agents_research
Research branch: research/jira-windows-agents (local; sem push)
Research artifact: ../research/windows-agents.md
Parent: ../map.md
Blocked by:

## Question

Quais fontes e mecanismos oficialmente disponíveis permitem acompanhar Limites de uso e Sessões de agente de Claude Code e Codex no Windows, preservando opt-in, encadeamento/reversão de hooks e status line, sem ler prompts/respostas ou renovar token Claude?

Distinguir execução nativa e WSL, formatos e caminhos, descoberta de processos/sessões vivas, transporte de hooks, permissões, quoting e uso do app-server. Apontar o que requer experimento futuro e como validá-lo no seam. Não consultar sessões, credenciais ou configurações reais da máquina; não instalar hooks. A pesquisa deve informar a decisão de escopo, sem presumir suporte simultâneo a WSL e execução nativa.

## Context

- `src-tauri/src/agents/`, `src-tauri/src/ai/`, `src-tauri/src/ports.rs`, `src-tauri/tests/hook_mode.rs` e ADR 0002.

## Answer

Pesquisa documental concluída em 03/11/2026. Codex e Claude Code têm caminhos documentados de execução nativa e WSL, com configuração, processos e fontes que devem permanecer distintos. O app-server oferece limites/estado do servidor conectado, sem comprovar inventário global de CLIs independentes. O Claude oferece interface JSON de sessões; estado background e processo vivo são dimensões diferentes, a discutir antes de mudar o domínio atual.

Hooks e status line têm mecanismos atuais para Windows, mas transporte Unix, quoting POSIX e wrapper `sh` do monitor exigem adaptação e validação. Preservam-se clique explícito, adição/reversão, privacidade e token Claude sem renovação. A escolha nativo/WSL e a cobertura mínima continuam abertas para o usuário.

Evidência e contexto: [IA e Sessões de agente no Windows](../research/windows-agents.md). Fontes locais e oficiais públicas revisadas; sem instalações, credenciais, sessões reais, mudanças em configurações ou testes nativos. Cenários de fixtures futuros estão identificados como propostas.
