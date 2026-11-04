# Quando a assinatura do app Windows é necessária?

Type: research
Labels: wayfinder:research
Status: resolved
Assignee: Codex / windows_signing_research
Research branch: research/jira-windows-signing (local; sem push)
Research artifact: ../research/windows-signing.md
Parent: ../map.md
Blocked by:

## Question

Para o Windows 11 x64 com instalador EXE por usuário, em quais condições assinar o executável e/ou instalador é requisito de execução, instalação, notificações ou distribuição, e em quais é uma questão de reputação ou política de segurança?

Verificar com fontes oficiais atuais Tauri/Microsoft: Authenticode, SmartScreen versus Smart App Control, instalação direta versus MSIX/Store, certificados confiáveis versus autoassinados, quais artefatos assinar e limites da atestação de proveniência GitHub. Investigar opções comprovadas de custo zero para projeto pessoal de código aberto, com elegibilidade e limitações, sem presumir concessão ou que assinatura elimina todo aviso. Conferir implicações para notificações nativas com ativação por clique. Não comprar certificado, registrar conta, aceitar termos ou configurar serviço.

## Context

- Solicitação explícita do usuário em 04/11/2026 durante [Qual é o mínimo obrigatório do produto no Windows?](05-windows-minimum.md).
- Alvo já aceito: Windows 11 x64; instalador EXE por usuário sem exigir administrador no fluxo normal; notificações nativas com clique para abrir Agentes; custo zero e sem servidor próprio.
- A pesquisa anterior em [Plataforma Windows nativa: capacidades, limites e verificações](../research/windows-platform.md) apontou limitações de distribuição sem assinatura. Esta investigação aprofunda sua necessidade antes de encerrar o contrato mínimo.
- Usar apenas fontes primárias públicas, distinguir exigência técnica, política e proposta. Nenhum certificado, chave privada, configuração do host ou dado real do usuário deve ser consultado.

## Comments

### Direcionamento complementar — 04/11/2026

O usuário pediu verificar um meio de dispensar a assinatura. Priorizar um caminho de distribuição que não exija certificado próprio nem etapa de assinatura pelo mantenedor, preservando custo zero e o fluxo por usuário. Distinguir EXE realmente não assinado de distribuição em que outro serviço assina; não presumir que portátil, winget, certificado autoassinado ou atestação GitHub eliminam exigências de confiança/políticas do Windows.

## Answer

Pesquisa concluída em 04/11/2026. Há caminho legítimo sem assinatura: distribuir diretamente EXE/NSIS por usuário. Authenticode não é requisito geral de execução de um app Tauri, mas SmartScreen pode advertir e Smart App Control ou políticas gerenciadas podem bloquear código desconhecido. Não existe garantia de execução em qualquer Windows 11; o produto não dependerá de desativar proteções ou importar confiança manualmente para cumprir o fluxo normal.

Notificações com ativação têm caminho oficial para app unpackaged e não impõem, por si só, MSIX ou Authenticode. O adaptador precisa ser validado para banner e Central; assinatura não corrige ausência de ativação. A rota Store/MSIX dispensa certificado próprio porque a Microsoft assina, mas não é artefato sem assinatura e exige outro canal/processo. Ofertas gratuitas de assinatura OSS têm condições e aprovação; não são pré-requisito da rota sem assinatura solicitada.

Evidência revisada: [Assinatura Windows: necessidade e caminho sem certificado próprio](../research/windows-signing.md), com fontes oficiais, cenários e limites. Pesquisa pública; sem inscrição, compra, certificado, dado do host, configuração, build ou teste. A rota inicial sem assinatura é incorporada ao contrato de [Qual é o mínimo obrigatório do produto no Windows?](05-windows-minimum.md), conforme o direcionamento explícito do usuário e os limites de instalação por política já aceitos.
