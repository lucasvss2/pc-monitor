# Limites de IA no Windows

Type: task
Status: claimed

## Escopo

Ativar o Módulo IA no Windows 11 x64 conforme a especificação de Jira/Windows. Em 04/11/2026, o usuário priorizou IA e adiou o aceite gráfico Trello/Ubuntu. A implementação compartilhada existente permanece preservada.

## Diagnóstico

O coletor estava condicionado a Linux e IA figurava entre os Módulos indisponíveis no Windows. O adaptador Codex usava um nome de comando sem tratar o launcher npm Windows; o wrapper Claude era POSIX. Não é necessário especular sobre falha da conta para explicar esse bloqueio explícito.

## Aceite

- Codex nativo lê limites pelo app-server existente, sem abrir terminal, criar conversa ou solicitar nova autenticação.
- Claude status line conecta apenas por clique, encadeia a anterior e restaura sem sobrescrever alterações de terceiros.
- OAuth opcional explicitamente indisponível no Windows; nenhum acesso às credenciais Claude nessa plataforma.
- Configurações distinguem fonte ausente, falta de leitura e indisponibilidade.
- Cache/logs só contêm metadados e limites. Payload bruto da status line nunca vai ao disco.
- CI nativa verifica processo e modo silencioso do EXE release; aceite gráfico permanece separado.

## Evidência inicial

`npm exec vitest -- run src/lib/ai/AiSettings.test.ts --pool=threads --maxWorkers=1`: 15 testes passaram e o novo teste falhou porque o opt-in ainda era oferecido. Após a mudança, 16 passaram. Rust local indisponível nesta máquina; a validação nativa usará CI, preservando as proteções do Windows.
