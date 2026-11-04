# Assinatura Windows: necessidade e caminho sem certificado próprio

Pesquisa em **04/11/2026**. Alvo: Windows 11 x64, Tauri v2, instalador EXE por usuário, uso pessoal, código aberto, custo zero e notificações nativas cujo clique abre Agentes. Somente documentação e código públicos; nenhuma conta, certificado, configuração do host, build ou teste consultado/executado.

## Resultado e recomendação

**É possível distribuir e executar o EXE diretamente sem Authenticode.** Tauri declara expressamente que assinatura não é requisito geral de execução. Isso permite manter o instalador por usuário e custo zero, aceitando avisos de reputação e indisponibilidade quando a política de segurança bloquear. Não permite prometer execução em qualquer Windows 11. [Tauri: assinatura Windows](https://v2.tauri.app/distribute/sign/windows/).

Recomendação para o mínimo já aceito: **EXE/NSIS por usuário, inicialmente sem assinatura, sem garantia de compatibilidade com políticas que bloqueiem código desconhecido**. Informar essa limitação na distribuição e mostrar diagnóstico quando o app conseguir detectar uma restrição; se o instalador nem iniciar, o aviso precisa estar na página de download. Não exigir desligar proteções, remover marca de origem ou importar certificado para instalar. Assinatura gratuita por fundação pode ser melhoria posterior, condicionada à aprovação; notificações continuam requisito independente.

## Matriz de necessidade

| Cenário | Natureza da exigência | Consequência |
| --- | --- | --- |
| EXE direto/NSIS em máquina pessoal que permite aplicativos Win32 | Assinatura recomendada, sem obrigação universal | Pode instalar/executar sem certificado; SmartScreen pode advertir. |
| SmartScreen | Reputação do arquivo e editor; política pode impedir prosseguir | Assinar reduz atrito e identifica editor, mas arquivo novo assinado também pode advertir. |
| Smart App Control (SAC) ativo | Controle de execução, inclusive arquivos sem download pelo navegador | Código desconhecido sem assinatura confiável pode ser bloqueado, sem exceção individual. |
| App Control/ambiente gerenciado | Política da organização | Mesmo assinatura pública não garante permissão; pode haver lista específica de editores/arquivos. |
| MSIX distribuído diretamente | Pacote deve estar assinado e ser confiável no dispositivo | Autoassinatura exige distribuição explícita de confiança; inadequada ao fluxo pessoal sem administração adicional. |
| Microsoft Store, submissão MSIX | Microsoft assina o pacote | Dispensa comprar/manter certificado próprio; continua sendo software assinado. |
| Microsoft Store, submissão MSI/EXE | Editor precisa assinar instalador e arquivos PE | Não é a mesma gratuidade de assinatura da rota MSIX. |

Fontes: [Tauri](https://v2.tauri.app/distribute/sign/windows/), [Microsoft: SmartScreen](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation), [Microsoft: SAC](https://support.microsoft.com/en-us/windows/security/threat-malware-protection/smart-app-control-frequently-asked-questions), [controle de aplicações](https://learn.microsoft.com/en-us/windows/security/book/application-security-application-and-driver-control), [MSIX](https://learn.microsoft.com/en-us/windows/msix/package/signing-package-overview), [rotas de distribuição](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/choose-distribution-path).

SmartScreen considera reputação do hash e da identidade assinante. **OV, EV e Artifact Signing não asseguram ausência de avisos**; EV deixou de receber confiança inicial automática. Certificado autoassinado se comporta como ausência de assinatura para esse fim. O prazo/limiar de reputação não é garantia contratual. [Microsoft: reputação](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation).

SAC primeiro avalia inteligência de segurança: permite se considerar seguro, bloqueia se considerar malicioso/indesejado; quando não consegue decidir, verifica assinatura confiável. Portanto nem “todo unsigned sempre bloqueia” nem “assinado ignora segurança” são conclusões corretas. Não há autorização individual de aplicativo bloqueado. [Microsoft: funcionamento SAC](https://support.microsoft.com/en-us/windows/security/threat-malware-protection/smart-app-control-frequently-asked-questions), [certificados aceitos](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control).

## Notificações e clique: não obrigam MSIX ou Authenticode

Há caminho oficial para aplicativos **unpackaged**: Windows App SDK `AppNotificationManager.Register()` registra o servidor COM que permite iniciar o aplicativo no clique. O guia trata notificações locais e não lista assinatura de código como pré-requisito; aplicativos elevados não são suportados. A documentação de ativação cobre aplicativo rodando e inicialização via COM. Isso sustenta um adaptador nativo sem MSIX obrigatório, sujeito a validar implantação do runtime, registro por usuário e integração Rust/Tauri. [Guia Microsoft](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/app-notifications-dotnet), [ativação](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/app-notifications-quickstart).

A [spec oficial do App SDK](https://github.com/microsoft/WindowsAppSDK/blob/main/specs/AppNotifications/AppNotifications-spec.md) inclui unpackaged, ativação e operações da Central. **Inferência de implementação:** o clique persistente pode ser realizado por esse adaptador, sem converter o produto para MSIX; ainda não foi demonstrado neste app. O caminho Win32 clássico também documenta atalho no menu Iniciar com AppUserModelID, sem requisito de certificado nessa configuração. [Microsoft: AUMID](https://learn.microsoft.com/en-us/windows/win32/shell/enable-desktop-toast-with-appusermodelid).

O [guia do plugin Tauri](https://v2.tauri.app/plugin/notification/) exige app instalado e ainda identifica Actions como mobile. Já o [código oficial da branch `v2`](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/notification/src/desktop.rs) consultado implementa `on_action`, mas documenta clique Windows somente no banner, não depois de ir à Central. Referência móvel, não versão de release garantida. **O projeto não possui `tauri-plugin-notification` nem `notify-rust` declarados/travados** em `src-tauri/Cargo.toml`, `Cargo.lock` e `package.json`; não há versão local à qual atribuir essa limitação. Assinar não adiciona ativação nem corrige essa diferença. O requisito de abrir Agentes não deve ser reduzido silenciosamente ao banner.

## Se houver assinatura: quais artefatos e qual confiança?

Assinar o executável instalado e o instalador final; para SAC, revisar todos os binários executados/carregados: auxiliares, DLLs, desinstalador e arquivos temporários do instalador. Assinatura só do invólucro não torna automaticamente confiável cada arquivo extraído. Preservar assinaturas de componentes de terceiros e verificar cobertura. Microsoft recomenda esse conjunto completo. [Microsoft: cobertura SAC](https://learn.microsoft.com/en-us/windows/security/book/application-security-application-and-driver-control).

Usar Authenticode e carimbo de tempo conforme o provedor; Tauri oferece `signCommand` para integração. [Tauri](https://v2.tauri.app/distribute/sign/windows/). Autoassinatura serve para testes/ambiente com confiança administrada; não cria confiança pública. Em MSIX autoassinado, instalar confiança no dispositivo é etapa separada. [Microsoft: autoassinatura](https://learn.microsoft.com/en-us/windows/msix/package/create-certificate-package-signing).

GitHub Artifact Attestations registra proveniência verificável do build, usando Sigstore; requer verificação pelo consumidor. **Não é Authenticode nem certificado de editor aceito pelo Windows** — conclusão pela diferença entre os mecanismos documentados. É complemento, assim como checksums; não substitui confiança pública. [GitHub](https://docs.github.com/en/actions/concepts/security/artifact-attestations), [Microsoft: assinatura SAC](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control).

## Opções com custo zero e condições

| Opção | Evidência e limites em 04/11/2026 |
| --- | --- |
| **Sem assinatura, EXE direto** | Custo zero e preserva formato aceito. Limite de segurança conforme matriz; portable/WinGet não transforma arquivo em confiável. |
| **SignPath Foundation** | Serviço e uso de certificado gratuitos para OSS; certificado em nome da fundação, chave em HSM. Não exige identificação pessoal do mantenedor segundo a apresentação. [Oferta](https://signpath.org/). |
| **OSSign** | Oferta gratuita, mas novas inscrições **suspensas por fila acumulada**. Exige código revisável, pipeline público, comunidade e mínimo de seis meses de atividade. [Fornecedor](https://ossign.org/). |
| **Necessary Code Signing** | Gratuito para OSS consolidado; exclui projetos novos usados pelo autor/poucos amigos, exige histórico público, revisão humana e documentos de identidade/endereço. Não é rota assegurada para app pessoal. [Fornecedor](https://sign.necessary.nu/). |
| **Store/MSIX** | Microsoft assina sem compra de certificado; nova inscrição individual sem taxa em quase 200 mercados, via fluxo específico, com documento e selfie. Elegibilidade do país e certificação permanecem pendentes. [Microsoft: cadastro](https://learn.microsoft.com/en-us/windows/apps/publish/whats-new-individual-developer), [MSIX versus EXE](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/choose-distribution-path). |

SignPath exige licença OSI sem dual licensing comercial, manutenção/release prévios, reputação verificável, MFA, papéis de revisão/aprovação, política pública, metadados e origem verificável; cada release exige aprovação manual. Sem SLA de aceitação identificado. Não foi encontrada lista conclusiva de países nos termos consultados; isso não comprova elegibilidade universal nem aprovação de mantenedor solo. [Termos](https://signpath.org/terms). Há integração CI e políticas GitHub Actions; será necessário analisar o pipeline após eventual concessão. [Integração](https://docs.signpath.io/build-system-integration), [políticas](https://docs.signpath.io/pipeline-policies/).

**Azure Artifact Signing, anteriormente Trusted Signing, é pago**, não solução permanente de custo zero: Microsoft informa início em US$ 9,99/mês. Public Trust atende organizações nos EUA, Canadá, UE e Reino Unido, e indivíduos nos EUA/Canadá; não presumir país ou enquadramento do mantenedor. [Preço e reputação](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation), [elegibilidade](https://learn.microsoft.com/en-us/windows/msix/package/signing-package-overview). Não considerar crédito promocional Azure como gratuidade recorrente.

## Verificação futura e limites

1. Em VM Windows 11 x64 descartável, instalar EXE por usuário sem elevação; documentar versão do Windows, SAC/políticas, download normal e resultado, sem alterar proteções para obter sucesso.
2. Validar adaptador de notificações: banner e Central, app aberto/oculto/encerrado, clique abrindo Agentes, instância única, atualização e desinstalação limpando registros; sem conteúdo privado nas notificações.
3. Se houver assinatura gratuita concedida, verificar cadeia, timestamp e cobertura de cada artefato final; repetir observação de SmartScreen/SAC sem prometer desaparecimento de avisos.

Pesquisa documental não confirma compatibilidade do binário ainda inexistente, concessão de fornecedor ou certificação Store. Nenhuma inscrição/compra/configuração foi feita. Para manter o escopo aceito, Store/MSIX permanece alternativa futura, **não mudança implícita do instalador EXE**.
