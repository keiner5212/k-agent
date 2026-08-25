import { useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { Select } from "@/components/Select";
import { useProvidersStore } from "@/lib/providers";
import {
  DEFAULT_BASE_URLS,
  PROVIDER_KINDS,
  type Provider,
  type ProviderKind,
} from "@/types/providers";

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
  const [apiKey, setApiKey] = useState("");
  const [clearApiKey, setClearApiKey] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [detectedCount, setDetectedCount] = useState<number | null>(null);

  const isEditing = Boolean(draft?.id);

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
    setSubmitting(true);
    setError(null);
    setDetectedCount(null);
    const result = await save({
      id: draft?.id,
      name: name.trim(),
      kind,
      baseUrl: baseUrl.trim(),
      apiKey: apiKey.trim() || undefined,
      clearApiKey,
    });
    setSubmitting(false);
    if (result.error) {
      setError(result.error);
      return;
    }
    if (result.provider) {
      setDetectedCount(result.provider.models.length);
    }
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
            onChange={(event) => {
              setApiKey(event.target.value);
              if (event.target.value.trim()) setClearApiKey(false);
            }}
            autoComplete="off"
            spellCheck={false}
            placeholder={
              isEditing && draft?.hasApiKey && !clearApiKey
                ? t("providers.form.apiKeyPlaceholderKeep")
                : t("providers.form.apiKeyPlaceholder")
            }
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

        {error ? (
          <div className="form-error" role="alert">
            {error}
          </div>
        ) : null}

        {detectedCount !== null ? (
          <div className="form-success" role="status">
            {t("providers.form.detected", { count: detectedCount })}
          </div>
        ) : null}
      </form>
    </Dialog>
  );
};
