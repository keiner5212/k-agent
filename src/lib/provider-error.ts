import type { ProviderErrorPayload } from "@/types/providers";

export type ProviderErrorTranslator = (key: string, options?: Record<string, unknown>) => string;

export const formatProviderError = (
  t: ProviderErrorTranslator,
  payload: ProviderErrorPayload,
): string => {
  switch (payload.kind) {
    case "timeout":
      return t("providers.errors.timeout", { seconds: payload.seconds });
    case "unreachable":
      return t("providers.errors.unreachable", { message: payload.message });
    case "invalidUrl":
      return t("providers.errors.invalidUrl", { url: payload.message });
    case "api": {
      const status = payload.status;
      if (status === 401 || status === 403) {
        return t("providers.errors.auth", { status });
      }
      if (status === 404) {
        return t("providers.errors.notFound", { status });
      }
      if (status === 429) {
        return t("providers.errors.rateLimited", { status });
      }
      if (status >= 500) {
        return t("providers.errors.serverError", { status });
      }
      if (status >= 400) {
        return t("providers.errors.clientError", { status });
      }
      return t("providers.errors.generic", { message: payload.message });
    }
    case "parse":
      return t("providers.errors.parse", { message: payload.message });
    case "http":
      return t("providers.errors.http", { message: payload.message });
    case "path":
    case "io":
    case "notFound":
    case "duplicate":
    case "crypto":
      return t("providers.errors.generic", { message: payload.message });
  }
};
