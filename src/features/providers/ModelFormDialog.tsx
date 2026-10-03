import { useCallback, useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { Loader2 } from "lucide-react";
import { Dialog } from "@/components/Dialog";
import { GlassButton } from "@/components/GlassButton";
import { Select } from "@/components/Select";
import { Toggle } from "@/components/Toggle";
import { isTauri } from "@/lib/platform";
import {
  formatContextWindow,
  parseTokenAmount,
  type ModelCost,
  type ModelDraft,
  type ModelInfo,
  type UserParamKind,
  type UserRequestParam,
} from "@/types/providers";

type ModelFormDialogProps = {
  open: boolean;
  model?: ModelInfo;
  onOpenChange: (open: boolean) => void;
  onSave: (draft: ModelDraft) => Promise<string | undefined>;
  onDelete?: () => Promise<string | undefined>;
};

type ParamRow = {
  key: string;
  name: string;
  kind: UserParamKind;
  values: string;
};

const KIND_OPTIONS = [
  { value: "string", label: "string" },
  { value: "number", label: "number" },
  { value: "bool", label: "bool" },
] as const;

const tokenField = (value: number | undefined): string => {
  if (value === undefined) return "";
  return formatContextWindow(value) === "-" ? String(value) : formatContextWindow(value);
};

const joinList = (values: string[] | undefined): string => (values ?? []).join(", ");

const splitList = (raw: string): string[] => {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const part of raw.split(",")) {
    const item = part.trim();
    if (!item || seen.has(item)) continue;
    seen.add(item);
    out.push(item);
  }
  return out;
};

const numberField = (value: number | undefined): string =>
  value === undefined ? "" : String(value);

const parseOptionalNumber = (raw: string): number | undefined | "invalid" => {
  const trimmed = raw.trim();
  if (!trimmed) return undefined;
  const value = Number(trimmed);
  if (!Number.isFinite(value) || value < 0) return "invalid";
  return value;
};

const parseCost = (
  inputRaw: string,
  outputRaw: string,
  reasoningRaw: string,
  cacheReadRaw: string,
  cacheWriteRaw: string,
  invalidMessage: string,
  setError: (message: string) => void,
): ModelCost | null | "invalid" => {
  const input = parseOptionalNumber(inputRaw);
  const output = parseOptionalNumber(outputRaw);
  const reasoning = parseOptionalNumber(reasoningRaw);
  const cacheRead = parseOptionalNumber(cacheReadRaw);
  const cacheWrite = parseOptionalNumber(cacheWriteRaw);
  if (
    input === "invalid" ||
    output === "invalid" ||
    reasoning === "invalid" ||
    cacheRead === "invalid" ||
    cacheWrite === "invalid"
  ) {
    setError(invalidMessage);
    return "invalid";
  }
  if (
    input === undefined &&
    output === undefined &&
    reasoning === undefined &&
    cacheRead === undefined &&
    cacheWrite === undefined
  ) {
    return null;
  }
  if (input === undefined || output === undefined) {
    setError(invalidMessage);
    return "invalid";
  }
  return {
    input,
    output,
    ...(reasoning === undefined ? {} : { reasoning }),
    ...(cacheRead === undefined ? {} : { cacheRead }),
    ...(cacheWrite === undefined ? {} : { cacheWrite }),
  };
};

const rowsFromParams = (params: UserRequestParam[], prefix: string): ParamRow[] =>
  params.map((param, index) => ({
    key: `${prefix}-${index}-${param.name}`,
    name: param.name,
    kind: param.kind,
    values: joinList(param.values),
  }));

const isKind = (value: string): value is UserParamKind =>
  value === "string" || value === "number" || value === "bool";

export const ModelFormDialog = ({
  open,
  model,
  onOpenChange,
  onSave,
  onDelete,
}: ModelFormDialogProps): ReactNode => {
  if (!open) return null;
  return (
    <ModelFormBody
      key={model?.id ?? "new"}
      model={model}
      onOpenChange={onOpenChange}
      onSave={onSave}
      onDelete={onDelete}
    />
  );
};

const ModelFormBody = ({
  model,
  onOpenChange,
  onSave,
  onDelete,
}: Omit<ModelFormDialogProps, "open">): ReactNode => {
  const { t } = useTranslation();
  const [id, setId] = useState(model?.id ?? "");
  const [displayName, setDisplayName] = useState(model?.displayName ?? "");
  const [family, setFamily] = useState(model?.family ?? "");
  const [contextRaw, setContextRaw] = useState(tokenField(model?.contextWindow));
  const [outputRaw, setOutputRaw] = useState(tokenField(model?.maxOutputTokens));
  const [inputRaw, setInputRaw] = useState(joinList(model?.input));
  const [outputModalities, setOutputModalities] = useState(joinList(model?.output));
  const [reasoning, setReasoning] = useState(model?.reasoning ?? false);
  const [toolCall, setToolCall] = useState(model?.toolCall ?? false);
  const [structuredOutput, setStructuredOutput] = useState(model?.structuredOutput ?? false);
  const [attachment, setAttachment] = useState(model?.attachment ?? false);
  const [multimodal, setMultimodal] = useState(model?.multimodal ?? false);
  const [costInput, setCostInput] = useState(numberField(model?.cost?.input));
  const [costOutput, setCostOutput] = useState(numberField(model?.cost?.output));
  const [costReasoning, setCostReasoning] = useState(numberField(model?.cost?.reasoning));
  const [costCacheRead, setCostCacheRead] = useState(numberField(model?.cost?.cacheRead));
  const [costCacheWrite, setCostCacheWrite] = useState(numberField(model?.cost?.cacheWrite));
  const [params, setParams] = useState<ParamRow[]>(() =>
    rowsFromParams(model?.request?.params ?? [], "saved"),
  );
  const paramsTouched = useRef(Boolean(model?.request?.params.length));
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadCatalogParams = useCallback((modelId: string): void => {
    if (!isTauri() || paramsTouched.current) return;
    const trimmed = modelId.trim();
    if (!trimmed) return;
    void invoke<UserRequestParam[]>("catalog_request_params", { modelId: trimmed }).then(
      (catalog) => {
        if (paramsTouched.current) return;
        setParams(rowsFromParams(catalog, "catalog"));
      },
      () => {
        if (!paramsTouched.current) setParams([]);
      },
    );
  }, []);

  useEffect(() => {
    loadCatalogParams(model?.id ?? "");
  }, [loadCatalogParams, model?.id]);

  const updateParam = (key: string, patch: Partial<ParamRow>): void => {
    paramsTouched.current = true;
    setParams((current) => current.map((row) => (row.key === key ? { ...row, ...patch } : row)));
  };

  const parseOptionalTokens = (raw: string, label: string): number | undefined | "invalid" => {
    if (!raw.trim()) return undefined;
    const parsed = parseTokenAmount(raw);
    if (parsed === undefined) {
      setError(t("providers.modelForm.errors.tokens", { field: label }));
      return "invalid";
    }
    return parsed;
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>): Promise<void> => {
    event.preventDefault();
    if (!id.trim()) {
      setError(t("providers.modelForm.errors.required"));
      return;
    }
    const contextWindow = parseOptionalTokens(contextRaw, t("providers.model.context"));
    if (contextWindow === "invalid") return;
    const maxOutputTokens = parseOptionalTokens(outputRaw, t("providers.model.output"));
    if (maxOutputTokens === "invalid") return;
    const cost = parseCost(
      costInput,
      costOutput,
      costReasoning,
      costCacheRead,
      costCacheWrite,
      t("providers.modelForm.errors.cost"),
      setError,
    );
    if (cost === "invalid") return;
    const nextParams: UserRequestParam[] = [];
    for (const row of params) {
      const name = row.name.trim();
      if (!name) continue;
      const values = row.kind === "bool" ? [] : splitList(row.values);
      if (row.kind === "number" && values.some((value) => !Number.isFinite(Number(value)))) {
        setError(t("providers.modelForm.errors.param"));
        return;
      }
      nextParams.push({ name, kind: row.kind, values });
    }

    setSubmitting(true);
    setError(null);
    const saveError = await onSave({
      originalId: model?.id,
      id: id.trim(),
      displayName: displayName.trim() || undefined,
      family: family.trim() || undefined,
      contextWindow,
      maxOutputTokens,
      input: splitList(inputRaw),
      output: splitList(outputModalities),
      reasoning,
      toolCall,
      structuredOutput,
      attachment,
      multimodal,
      cost: cost ?? undefined,
      request: nextParams.length > 0 ? { params: nextParams } : undefined,
    });
    setSubmitting(false);
    if (saveError) {
      setError(saveError);
      return;
    }
    onOpenChange(false);
  };

  const handleDelete = async (): Promise<void> => {
    if (!onDelete) return;
    setSubmitting(true);
    setError(null);
    const deleteError = await onDelete();
    setSubmitting(false);
    if (deleteError) {
      setError(deleteError);
      return;
    }
    onOpenChange(false);
  };

  const isEditing = Boolean(model);

  return (
    <Dialog
      open
      onOpenChange={onOpenChange}
      titleKey={isEditing ? "providers.modelForm.editTitle" : "providers.modelForm.createTitle"}
      size="wide"
      placement="center"
      footer={
        <>
          {isEditing && onDelete ? (
            <GlassButton variant="danger" onClick={() => void handleDelete()} disabled={submitting}>
              {t("providers.actions.delete")}
            </GlassButton>
          ) : null}
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
            form="model-form"
            disabled={submitting || !id.trim()}
          >
            {submitting ? (
              <>
                <Loader2 size={14} strokeWidth={1.5} className="spin" />
                <span>{t("providers.modelForm.saving")}</span>
              </>
            ) : (
              <span>{t("providers.form.save")}</span>
            )}
          </GlassButton>
        </>
      }
    >
      <form
        id="model-form"
        className="skill-form model-form model-form--dialog"
        onSubmit={(event) => void handleSubmit(event)}
      >
        <section className="model-form__section">
          <h3>{t("providers.modelForm.sharedTitle")}</h3>
          <div className="field">
            <label className="field__label" htmlFor="model-id">
              {t("providers.modelForm.id")}
            </label>
            <input
              id="model-id"
              className="input input--mono"
              value={id}
              onChange={(event) => setId(event.target.value)}
              onBlur={() => {
                loadCatalogParams(id);
              }}
              autoComplete="off"
              spellCheck={false}
              required
            />
          </div>
          <div className="field">
            <label className="field__label" htmlFor="model-name">
              {t("providers.modelForm.displayName")}
            </label>
            <input
              id="model-name"
              className="input"
              value={displayName}
              onChange={(event) => setDisplayName(event.target.value)}
              autoComplete="off"
            />
          </div>
          <div className="field">
            <label className="field__label" htmlFor="model-family">
              {t("providers.modelForm.family")}
            </label>
            <input
              id="model-family"
              className="input"
              value={family}
              onChange={(event) => setFamily(event.target.value)}
              autoComplete="off"
              spellCheck={false}
            />
            <span className="field__hint">{t("providers.modelForm.familyHint")}</span>
          </div>
          <div className="model-form__row">
            <div className="field">
              <label className="field__label" htmlFor="model-context">
                {t("providers.model.context")}
              </label>
              <input
                id="model-context"
                className="input input--mono"
                value={contextRaw}
                onChange={(event) => setContextRaw(event.target.value)}
                autoComplete="off"
                spellCheck={false}
                placeholder="512k"
              />
            </div>
            <div className="field">
              <label className="field__label" htmlFor="model-output">
                {t("providers.model.output")}
              </label>
              <input
                id="model-output"
                className="input input--mono"
                value={outputRaw}
                onChange={(event) => setOutputRaw(event.target.value)}
                autoComplete="off"
                spellCheck={false}
                placeholder="64k"
              />
            </div>
          </div>
          <p className="field__hint">{t("providers.modelForm.tokenHint")}</p>
          <div className="model-form__row">
            <div className="field">
              <label className="field__label" htmlFor="model-input">
                {t("providers.modelForm.input")}
              </label>
              <input
                id="model-input"
                className="input input--mono"
                value={inputRaw}
                onChange={(event) => setInputRaw(event.target.value)}
                autoComplete="off"
                spellCheck={false}
                placeholder="text, image"
              />
            </div>
            <div className="field">
              <label className="field__label" htmlFor="model-output-modalities">
                {t("providers.modelForm.output")}
              </label>
              <input
                id="model-output-modalities"
                className="input input--mono"
                value={outputModalities}
                onChange={(event) => setOutputModalities(event.target.value)}
                autoComplete="off"
                spellCheck={false}
                placeholder="text"
              />
            </div>
          </div>
          <p className="field__hint">{t("providers.modelForm.listHint")}</p>
          <Toggle
            checked={reasoning}
            onChange={setReasoning}
            label={t("providers.modelForm.reasoning")}
          />
          <Toggle
            checked={toolCall}
            onChange={setToolCall}
            label={t("providers.modelForm.toolCall")}
          />
          <Toggle
            checked={structuredOutput}
            onChange={setStructuredOutput}
            label={t("providers.modelForm.structuredOutput")}
          />
          <Toggle
            checked={attachment}
            onChange={setAttachment}
            label={t("providers.modelForm.attachment")}
          />
          <Toggle
            checked={multimodal}
            onChange={setMultimodal}
            label={t("providers.model.multimodal")}
            description={t("providers.modelForm.multimodalHint")}
          />
          <div className="model-form__row">
            <div className="field">
              <label className="field__label" htmlFor="model-cost-input">
                {t("providers.modelForm.costInput")}
              </label>
              <input
                id="model-cost-input"
                className="input input--mono"
                inputMode="decimal"
                value={costInput}
                onChange={(event) => setCostInput(event.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
            <div className="field">
              <label className="field__label" htmlFor="model-cost-output">
                {t("providers.modelForm.costOutput")}
              </label>
              <input
                id="model-cost-output"
                className="input input--mono"
                inputMode="decimal"
                value={costOutput}
                onChange={(event) => setCostOutput(event.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
          </div>
          <div className="model-form__row model-form__row--three">
            <div className="field">
              <label className="field__label" htmlFor="model-cost-reasoning">
                {t("providers.modelForm.costReasoning")}
              </label>
              <input
                id="model-cost-reasoning"
                className="input input--mono"
                inputMode="decimal"
                value={costReasoning}
                onChange={(event) => setCostReasoning(event.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
            <div className="field">
              <label className="field__label" htmlFor="model-cost-read">
                {t("providers.modelForm.costCacheRead")}
              </label>
              <input
                id="model-cost-read"
                className="input input--mono"
                inputMode="decimal"
                value={costCacheRead}
                onChange={(event) => setCostCacheRead(event.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
            <div className="field">
              <label className="field__label" htmlFor="model-cost-write">
                {t("providers.modelForm.costCacheWrite")}
              </label>
              <input
                id="model-cost-write"
                className="input input--mono"
                inputMode="decimal"
                value={costCacheWrite}
                onChange={(event) => setCostCacheWrite(event.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
            </div>
          </div>
          <p className="field__hint">{t("providers.modelForm.costHint")}</p>
        </section>
        <section className="model-form__section">
          <h3>{t("providers.modelForm.requestTitle")}</h3>
          <p className="field__hint">{t("providers.modelForm.requestHint")}</p>
          {params.map((row) => (
            <div className="model-form__param" key={row.key}>
              <input
                className="input input--mono"
                value={row.name}
                aria-label={t("providers.modelForm.paramName")}
                onChange={(event) => updateParam(row.key, { name: event.target.value })}
                autoComplete="off"
                spellCheck={false}
              />
              <Select
                value={row.kind}
                options={KIND_OPTIONS}
                menuMinWidth={144}
                ariaLabel={t("providers.modelForm.paramKind")}
                onChange={(next) => {
                  if (isKind(next)) updateParam(row.key, { kind: next });
                }}
              />
              {row.kind === "bool" ? (
                <span className="field__hint">{t("providers.modelForm.paramBool")}</span>
              ) : (
                <input
                  className="input input--mono"
                  value={row.values}
                  aria-label={t("providers.modelForm.paramValues")}
                  placeholder={t("providers.modelForm.paramValues")}
                  onChange={(event) => updateParam(row.key, { values: event.target.value })}
                  autoComplete="off"
                  spellCheck={false}
                />
              )}
              <GlassButton
                variant="ghost"
                onClick={() => {
                  paramsTouched.current = true;
                  setParams((current) => current.filter((item) => item.key !== row.key));
                }}
              >
                {t("providers.modelForm.removeParam")}
              </GlassButton>
            </div>
          ))}
          <GlassButton
            variant="secondary"
            onClick={() => {
              paramsTouched.current = true;
              setParams((current) => [
                ...current,
                {
                  key: `new-${current.length}-${Date.now()}`,
                  name: "",
                  kind: "string",
                  values: "",
                },
              ]);
            }}
          >
            {t("providers.modelForm.addParam")}
          </GlassButton>
        </section>
        {error ? (
          <div className="form-error" role="alert">
            {error}
          </div>
        ) : null}
      </form>
    </Dialog>
  );
};
