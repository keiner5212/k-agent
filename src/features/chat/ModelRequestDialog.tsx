import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { SlidersHorizontal } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { IconButton } from "@/components/IconButton";
import { Select } from "@/components/Select";
import { resolveSelectedModel } from "@/lib/context-usage";
import { isTauri } from "@/lib/platform";
import { useProvidersStore } from "@/lib/providers";
import { selectRequest, useSelectionStore } from "@/lib/selected-model";
import { useSettingsStore } from "@/lib/settings";
import type { ModelRequestOverride, ModelRequestView } from "@/types/model-request";

const optionList = (
  values: readonly string[],
  current: string,
  defaultLabel: string,
): { value: string; label: string }[] => [
  { value: "", label: defaultLabel },
  ...values.map((value) => ({ value, label: value })),
  ...(current && !values.includes(current) ? [{ value: current, label: current }] : []),
];

export const ModelRequestButton = (): ReactNode => {
  const { t } = useTranslation();
  const selection = useSelectionStore((state) => state.selection);
  const providers = useProvidersStore((state) => state.providers);
  const stored = useSelectionStore(selectRequest);
  const setRequest = useSelectionStore((state) => state.setRequest);
  const limitProviderDataUse = useSettingsStore((state) => state.limitProviderDataUse);
  const [open, setOpen] = useState(false);
  const [profile, setProfile] = useState<ModelRequestView | null>(null);
  const [error, setError] = useState<string | undefined>();
  const model = resolveSelectedModel(providers, selection);

  useEffect(() => {
    if (!open || !model || !selection || !isTauri()) return;
    let cancelled = false;
    void invoke<ModelRequestView>("describe_model_request", {
      query: {
        kind: providers.find((item) => item.id === selection.providerId)?.kind,
        baseUrl: providers.find((item) => item.id === selection.providerId)?.baseUrl,
        modelId: model.id,
      },
    })
      .then((next) => {
        if (!cancelled) setProfile(next);
      })
      .catch((reason: unknown) => {
        if (!cancelled) setError(reason instanceof Error ? reason.message : String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [model, open, providers, selection]);

  if (!model || !selection) return null;

  const update = (patch: ModelRequestOverride): void => {
    setRequest({ ...stored, ...patch });
  };

  return (
    <>
      <IconButton
        label={t("chat.request.open")}
        onClick={() => {
          setError(undefined);
          setProfile(null);
          setOpen(true);
        }}
      >
        <SlidersHorizontal size={16} strokeWidth={1.5} />
      </IconButton>
      <Dialog
        open={open}
        onOpenChange={setOpen}
        titleKey="chat.request.title"
        placement="center"
        size="narrow"
      >
        <div className="model-request">
          {error ? <p className="form-error">{error}</p> : null}
          {profile ? (
            <RequestForm
              profile={profile}
              stored={stored}
              limitProviderDataUse={limitProviderDataUse}
              onChange={update}
            />
          ) : null}
        </div>
      </Dialog>
    </>
  );
};

const RequestForm = ({
  profile,
  stored,
  limitProviderDataUse,
  onChange,
}: {
  profile: ModelRequestView;
  stored: ModelRequestOverride;
  limitProviderDataUse: boolean;
  onChange: (patch: ModelRequestOverride) => void;
}): ReactNode => {
  const { t } = useTranslation();
  const defaultLabel = t("chat.request.default");
  const reasoning = profile.reasoning;

  return (
    <>
      <p className="field__hint">
        {profile.modelContract === "verified"
          ? t("chat.request.contractVerified")
          : t("chat.request.contractUnknown")}
      </p>
      {reasoning.kind === "thinking" || reasoning.kind === "gemini-budget" ? (
        <Choice
          id="request-thinking"
          label={t("chat.request.thinking")}
          value={stored.reasoningMode ?? ""}
          options={optionList(
            reasoning.modes,
            stored.reasoningMode ?? "",
            `${defaultLabel} (${reasoning.defaultMode})`,
          )}
          disabled={reasoning.lockedOn}
          onChange={(reasoningMode) => onChange({ reasoningMode: reasoningMode || undefined })}
        />
      ) : null}
      {reasoning.kind === "claude" ? (
        <>
          <Choice
            id="request-thinking"
            label={t("chat.request.thinking")}
            value={stored.reasoningMode ?? ""}
            options={optionList(
              reasoning.thinkingModes,
              stored.reasoningMode ?? "",
              `${defaultLabel} (${reasoning.defaultThinking})`,
            )}
            disabled={reasoning.lockedOn}
            onChange={(reasoningMode) => onChange({ reasoningMode: reasoningMode || undefined })}
          />
          <Choice
            id="request-effort"
            label={t("chat.request.effort")}
            value={stored.effort ?? ""}
            options={optionList(
              reasoning.levels,
              stored.effort ?? "",
              reasoning.defaultLevel ? `${defaultLabel} (${reasoning.defaultLevel})` : defaultLabel,
            )}
            onChange={(effort) => onChange({ effort: effort || undefined })}
          />
        </>
      ) : null}
      {reasoning.kind === "effort" || reasoning.kind === "gemini-level" ? (
        <Choice
          id="request-effort"
          label={t("chat.request.effort")}
          value={stored.effort ?? ""}
          options={optionList(
            reasoning.levels,
            stored.effort ?? "",
            reasoning.defaultLevel ? `${defaultLabel} (${reasoning.defaultLevel})` : defaultLabel,
          )}
          onChange={(effort) => onChange({ effort: effort || undefined })}
        />
      ) : null}
      {reasoning.kind === "unknown" ? (
        <p className="field__hint">{t("chat.request.reasoningUnknown")}</p>
      ) : null}
      {profile.serviceTiers.length > 0 ? (
        <Choice
          id="request-tier"
          label={t("chat.request.serviceTier")}
          value={stored.serviceTier ?? ""}
          options={optionList(profile.serviceTiers, stored.serviceTier ?? "", defaultLabel)}
          onChange={(serviceTier) => onChange({ serviceTier: serviceTier || undefined })}
        />
      ) : null}
      {profile.sampling.temperature === "range" ? (
        <div className="field">
          <label className="field__label" htmlFor="request-temperature">
            {t("chat.request.temperature")}
          </label>
          <input
            id="request-temperature"
            className="input input--mono"
            inputMode="decimal"
            value={stored.temperature ?? ""}
            placeholder={String(profile.sampling.defaultValue ?? "")}
            onChange={(event) => {
              const raw = event.target.value.trim();
              if (!raw) {
                onChange({ temperature: undefined });
                return;
              }
              const temperature = Number(raw);
              if (Number.isFinite(temperature)) onChange({ temperature });
            }}
          />
        </div>
      ) : null}
      {profile.reasoningSplit ? (
        <p className="field__hint">{t("chat.request.note.reasoningSplit")}</p>
      ) : null}
      {profile.notes.map((note) => (
        <p className="field__hint" key={note}>
          {t(note)}
        </p>
      ))}
      <p className="field__hint">
        {limitProviderDataUse
          ? t("chat.request.privacyOn", { detail: t(profile.privacyDetailKey) })
          : t("chat.request.privacyOff")}
      </p>
    </>
  );
};

const Choice = ({
  id,
  label,
  value,
  options,
  disabled,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  options: { value: string; label: string }[];
  disabled?: boolean;
  onChange: (value: string) => void;
}): ReactNode => (
  <div className="field">
    <label className="field__label" htmlFor={id}>
      {label}
    </label>
    <Select id={id} value={value} options={options} onChange={onChange} disabled={disabled} />
  </div>
);
