const messages: Record<string, string> = {
  invalidCredential: "Credencial inválida ou expirada. Confira os dados da conexão.",
  invalidSite: "Informe o endereço HTTPS do seu site Jira Cloud, por exemplo https://exemplo.atlassian.net.",
  offline: "Não foi possível consultar o serviço. Confira a conexão e tente novamente.",
  keyringUnavailable: "O cofre do sistema está indisponível. A alteração não foi concluída.",
  cleanupFailed: "Não foi possível concluir a limpeza. A coleta está pausada e os dados anteriores estão ocultos.",
  settingsUnavailable: "As preferências de Tarefas não puderam ser carregadas com segurança. Atualize o aplicativo ou restaure suas preferências.",
  staleContext: "A fonte ou o quadro mudou. Confira a leitura atual antes de tentar novamente.",
  readRequired: "Atualize o quadro antes de fazer outra alteração.",
  unconfirmed: "Resultado não confirmado. A alteração pode ter sido aplicada. Atualize o quadro antes de tentar novamente.",
  requiredFields: "Essa transição exige campos adicionais. Continue no Jira.",
  refused: "O serviço recusou a ação ou você não tem acesso. Atualize o quadro e confira no serviço.",
  unsupportedBoard: "Não foi possível ler o quadro completo e suas colunas. Confira o acesso e a configuração no Jira.",
};
export function taskError(error: unknown): string { return messages[typeof error === "string" ? error : ""] ?? "Não foi possível concluir a ação. Atualize o quadro e tente novamente."; }
