import { useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2, X } from "lucide-react";
import { IconButton } from "@/components/IconButton";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { Select } from "@/components/Select";
import { useProvidersStore } from "@/lib/providers";
import { formatProviderError } from "@/lib/provider-error";
import { clearStoredSecretMask, isStoredSecretMask, storedSecretDisplay } from "@/lib/secret-field";
import {
  DEFAULT_BASE_URLS,
  PROVIDER_KINDS,
  type Provider,
  type ProviderHeader,
  type ProviderHeaderKind,
  type ProviderKind,
} from "@/types/providers";

const MAX_HEADERS = 32;
const HEADER_NAME = /^[!#$%&'*+\-.0-9A-Za-z^_`|~]+$/;
const HEADER_KINDS: readonly ProviderHeaderKind[] = ["defined", "generated"];

type HeaderRow = {
  key: string;
  name: string;
  kind: ProviderHeaderKind;
  value: string;
};

const nextHeaderKey = (): string =>
  typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `header-${Date.now()}-${Math.random().toString(16).slice(2)}`;

const rowsFromHeaders = (headers: ProviderHeader[] | undefined): HeaderRow[] =>
  (headers ?? []).map((header) => ({
    key: nextHeaderKey(),
    name: header.name,
    kind: header.kind ?? "defined",
    value: header.value,
  }));

const parseHeaderRows = (
  rows: HeaderRow[],
): { headers: ProviderHeader[] } | { error: "name" | "empty" | "duplicate" | "limit" } => {
  const headers: ProviderHeader[] = [];
  const seen = new Set<string>();
  for (const row of rows) {
    const name = row.name.trim();
    const value = row.value.trim();
    if (!name && !value) continue;
    if (!name || !value) return { error: "empty" };
    if (!HEADER_NAME.test(name) || name.length > 128) return { error: "name" };
    const key = name.toLowerCase();
    if (seen.has(key)) return { error: "duplicate" };
    seen.add(key);
    if (headers.length >= MAX_HEADERS) return { error: "limit" };
    headers.push({ name, kind: row.kind, value });
  }
  return { headers };
};

type ProviderFormDialogProps = {
  open: boolean;
  draft?: Provider;
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
};

export const ProviderFormDialog = ({
  open,
  draft,
  onOpenChange,
  onSaved,
}: ProviderFormDialogProps): ReactNode => {
  if (!open) return null;
  return (
    <ProviderFormBody
      key={draft?.id ?? "create"}
      draft={draft}
      onOpenChange={onOpenChange}
      onSaved={onSaved}
    />
  );
};

type ProviderFormBodyProps = {
  draft?: Provider;
  onOpenChange: (open: boolean) => void;
  onSaved: () => void;
};

const ProviderFormBody = ({ draft, onOpenChange, onSaved }: ProviderFormBodyProps): ReactNode => {
  const { t } = useTranslation();
  const save = useProvidersStore((state) => state.save);
  const [name, setName] = useState(draft?.name ?? "");
  const [kind, setKind] = useState<ProviderKind>(draft?.kind ?? "openai-like");
  const [baseUrl, setBaseUrl] = useState(draft?.baseUrl ?? DEFAULT_BASE_URLS["openai-like"]);
  const [apiKey, setApiKey] = useState(storedSecretDisplay(draft?.hasApiKey));
  const [clearApiKey, setClearApiKey] = useState(false);
  const [headerRows, setHeaderRows] = useState<HeaderRow[]>(() => rowsFromHeaders(draft?.headers));
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [detectedCount, setDetectedCount] = useState<number | null>(null);

  const isEditing = Boolean(draft?.id);

  const patchHeader = (key: string, patch: Partial<HeaderRow>): void => {
    setHeaderRows((rows) => rows.map((item) => (item.key === key ? { ...item, ...patch } : item)));
  };

  const handleKindChange = (next: ProviderKind): void => {
    setKind(next);
    if (!isEditing && baseUrl === DEFAULT_BASE_URLS[kind]) {
      setBaseUrl(DEFAULT_BASE_URLS[next]);
    }
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>): Promise<void> => {
    event.preventDefault();
    if (!name.trim() || !baseUrl.trim()) {
      setError(t("providers.form.errors.required"));
      return;
    }
    const parsedHeaders = parseHeaderRows(headerRows);
    if ("error" in parsedHeaders) {
      const key = {
        name: "providers.form.errors.headerName",
        empty: "providers.form.errors.headerEmpty",
        duplicate: "providers.form.errors.headerDuplicate",
        limit: "providers.form.errors.headerLimit",
      }[parsedHeaders.error];
      setError(t(key));
      return;
    }
    setSubmitting(true);
    setSuccess(null);
    setError(null);
    setDetectedCount(null);
    const result = await save({
      id: draft?.id,
      name: name.trim(),
      kind,
      baseUrl: baseUrl.trim(),
      apiKey:
        isStoredSecretMask(apiKey) || (apiKey.trim() === "" && draft?.hasApiKey && !clearApiKey)
          ? undefined
          : apiKey.trim() || undefined,
      clearApiKey,
      headers: parsedHeaders.headers,
    });
    setSubmitting(false);
    if (result.error) {
      setError(
        result.errorPayload
          ? formatProviderError(t, result.errorPayload)
          : t("providers.form.errors.fetchFailed", { message: result.error }),
      );
      return;
    }
    if (!result.provider) {
      setError(t("providers.form.errors.unknown"));
      return;
    }
    if (!isEditing && result.provider.models.length === 0) {
      setError(t("providers.form.errors.noModels"));
      return;
    }
    setDetectedCount(result.provider.models.length);
    setSuccess(t("providers.form.success", { count: result.provider.models.length }));
    onSaved();
    onOpenChange(false);
  };

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      titleKey={isEditing ? "providers.form.editTitle" : "providers.form.createTitle"}
      size="default"
      placement="center"
      footer={
        <>
          <GlassButton
            variant="secondary"
            onClick={() => onOpenChange(false)}
            disabled={submitting}
          >
            {t("providers.form.cancel")}
          </GlassButton>
          <GlassButton
            variant="primary"
            type="submit"
            form="provider-form"
            disabled={submitting || !name.trim() || !baseUrl.trim()}
          >
            {submitting ? (
              <>
                <Loader2 size={14} strokeWidth={1.5} className="spin" />
                <span>{t("providers.form.detecting")}</span>
              </>
            ) : (
              <span>{isEditing ? t("providers.form.save") : t("providers.form.add")}</span>
            )}
          </GlassButton>
        </>
      }
    >
      <form
        id="provider-form"
        className="skill-form provider-form"
        onSubmit={(event) => void handleSubmit(event)}
      >
        <div className="field">
          <label className="field__label" htmlFor="provider-name">
            {t("providers.form.name")}
          </label>
          <input
            id="provider-name"
            className="input"
            type="text"
            value={name}
            onChange={(event) => setName(event.target.value)}
            autoComplete="off"
            required
          />
        </div>

        <div className="field">
          <label className="field__label" htmlFor="provider-kind">
            {t("providers.form.kind")}
          </label>
          <Select
            id="provider-kind"
            value={kind}
            onChange={(next) => handleKindChange(next as ProviderKind)}
            options={PROVIDER_KINDS.map((value) => ({
              value,
              label: t(`providers.kinds.${value}`),
            }))}
          />
          <span className="field__hint">{t(`providers.form.kindHint.${kind}`)}</span>
        </div>

        <div className="field">
          <label className="field__label" htmlFor="provider-base-url">
            {t("providers.form.baseUrl")}
          </label>
          <input
            id="provider-base-url"
            className="input input--mono"
            type="url"
            value={baseUrl}
            onChange={(event) => setBaseUrl(event.target.value)}
            autoComplete="off"
            spellCheck={false}
            required
          />
        </div>

        <div className="field">
          <label className="field__label" htmlFor="provider-api-key">
            {t("providers.form.apiKey")}
          </label>
          <input
            id="provider-api-key"
            className="input"
            type="password"
            value={apiKey}
            onFocus={() => clearStoredSecretMask(apiKey, () => setApiKey(""))}
            onChange={(event) => {
              setApiKey(event.target.value);
              if (event.target.value.trim()) setClearApiKey(false);
            }}
            autoComplete="off"
            spellCheck={false}
            placeholder={t("providers.form.apiKeyPlaceholder")}
          />
          <span className="field__hint">{t("providers.form.apiKeyHint")}</span>
          {isEditing && draft?.hasApiKey && !clearApiKey ? (
            <button
              type="button"
              className="field__action"
              onClick={() => {
                setClearApiKey(true);
                setApiKey("");
              }}
            >
              {t("providers.form.apiKeyRemove")}
            </button>
          ) : null}
        </div>

        <div className="field">
          <span className="field__label" id="provider-headers-label">
            {t("providers.form.headers")}
          </span>
          <div
            className="provider-form__headers"
            role="group"
            aria-labelledby="provider-headers-label"
          >
            {headerRows.map((row) => (
              <div className="provider-form__header-row" key={row.key}>
                <input
                  className="input"
                  type="text"
                  value={row.name}
                  aria-label={t("providers.form.headerName")}
                  placeholder={t("providers.form.headerName")}
                  autoComplete="off"
                  spellCheck={false}
                  onChange={(event) => patchHeader(row.key, { name: event.target.value })}
                />
                <Select
                  value={row.kind}
                  ariaLabel={t("providers.form.headerKind")}
                  onChange={(next) => patchHeader(row.key, { kind: next as ProviderHeaderKind })}
                  options={HEADER_KINDS.map((value) => ({
                    value,
                    label: t(`providers.form.headerKinds.${value}`),
                  }))}
                />
                <input
                  className="input input--mono"
                  type="text"
                  value={row.value}
                  aria-label={
                    row.kind === "generated"
                      ? t("providers.form.headerPattern")
                      : t("providers.form.headerValue")
                  }
                  placeholder={
                    row.kind === "generated"
                      ? t("providers.form.headerPatternPlaceholder")
                      : t("providers.form.headerValuePlaceholder")
                  }
                  autoComplete="off"
                  spellCheck={false}
                  onChange={(event) => patchHeader(row.key, { value: event.target.value })}
                />
                <IconButton
                  label={t("providers.form.removeHeader")}
                  onClick={() =>
                    setHeaderRows((rows) => rows.filter((item) => item.key !== row.key))
                  }
                >
                  <X size={14} strokeWidth={1.5} />
                </IconButton>
              </div>
            ))}
          </div>
          <GlassButton
            variant="secondary"
            type="button"
            disabled={headerRows.length >= MAX_HEADERS}
            onClick={() =>
              setHeaderRows((rows) => [
                ...rows,
                { key: nextHeaderKey(), name: "", kind: "defined", value: "" },
              ])
            }
          >
            {t("providers.form.addHeader")}
          </GlassButton>
          <span className="field__hint">{t("providers.form.headersHint")}</span>
        </div>

        {error ? (
          <div className="form-error" role="alert">
            {error}
          </div>
        ) : null}

        {success ? (
          <div className="form-success" role="status">
            {t("providers.form.detected", { count: detectedCount })}
          </div>
        ) : null}
      </form>
    </Dialog>
  );
};
