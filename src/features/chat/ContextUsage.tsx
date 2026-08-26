import { useMemo, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";
import { useAgentsStore } from "@/lib/agents";
import { resolveAgentPersonality } from "@/lib/builtin-agents";
import { useComposerStore } from "@/lib/composer";
import {
  buildContextUsage,
  formatUsageCost,
  formatUsageTokens,
  resolveSelectedModel,
} from "@/lib/context-usage";
import { estimateTokensFromText } from "@/lib/jobs-handlers";
import { useProvidersStore } from "@/lib/providers";
import { responseLanguageDirective } from "@/lib/response-language";
import { selectActiveMessages, useSessionsStore } from "@/lib/sessions";
import { useSelectionStore } from "@/lib/selected-model";
import { useSettingsStore } from "@/lib/settings";
import { formatContextWindow } from "@/types/providers";

const RING_SIZE = 14;
const RING_STROKE = 2;
const RING_RADIUS = (RING_SIZE - RING_STROKE) / 2;
const RING_CENTER = RING_SIZE / 2;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

export const ContextUsage = (): ReactNode => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const selection = useSelectionStore((state) => state.selection);
  const providers = useProvidersStore((state) => state.providers);
  const messages = useSessionsStore(selectActiveMessages);
  const selectedAgent = useComposerStore((state) => state.selectedAgent);
  const agentContexts = useAgentsStore((state) => state.contexts);
  const forceResponseLanguage = useSettingsStore((state) => state.forceResponseLanguage);
  const responseLanguage = useSettingsStore((state) => state.responseLanguage);
  const model = useMemo(() => resolveSelectedModel(providers, selection), [providers, selection]);
  const systemPromptTokens = useMemo(
    () => estimateTokensFromText(resolveAgentPersonality(selectedAgent, agentContexts, t)),
    [agentContexts, selectedAgent, t],
  );
  const languageDirectiveTokens = useMemo(
    () =>
      forceResponseLanguage
        ? estimateTokensFromText(responseLanguageDirective(responseLanguage))
        : 0,
    [forceResponseLanguage, responseLanguage],
  );
  const usage = useMemo(
    () =>
      buildContextUsage({
        windowTokens: model?.contextWindow,
        extras: { systemPrompt: systemPromptTokens, languageDirective: languageDirectiveTokens },
        cost: model?.cost,
        messages,
      }),
    [languageDirectiveTokens, messages, model, systemPromptTokens],
  );
  const visibleBuckets = usage.buckets.filter((item) => item.tokens > 0);
  const listBuckets =
    visibleBuckets.length > 0
      ? visibleBuckets
      : usage.buckets.filter((item) => item.id === "conversation");
  const cost = formatUsageCost(usage.costUsd);
  const usedLabel = formatUsageTokens(usage.usedTokens);
  const windowLabel =
    usage.windowTokens > 0 ? formatUsageTokens(usage.windowTokens) : formatContextWindow(undefined);
  const offset = RING_CIRCUMFERENCE * (1 - Math.min(100, Math.max(0, usage.percent)) / 100);

  return (
    <>
      <button
        type="button"
        className="context-usage"
        onClick={() => setOpen(true)}
        aria-label={t("chat.usage.label", { percent: usage.percent, cost })}
      >
        <span className="context-usage__ring-wrap">
          <svg
            className="context-usage__ring"
            viewBox={`0 0 ${RING_SIZE} ${RING_SIZE}`}
            aria-hidden="true"
          >
            <circle
              className="context-usage__track"
              cx={RING_CENTER}
              cy={RING_CENTER}
              r={RING_RADIUS}
              fill="none"
              strokeWidth={RING_STROKE}
            />
            <circle
              className="context-usage__fill"
              cx={RING_CENTER}
              cy={RING_CENTER}
              r={RING_RADIUS}
              fill="none"
              strokeWidth={RING_STROKE}
              strokeDasharray={RING_CIRCUMFERENCE}
              strokeDashoffset={offset}
              transform={`rotate(-90 ${RING_CENTER} ${RING_CENTER})`}
            />
          </svg>
        </span>
        <span className="context-usage__copy">
          <span className="context-usage__line">
            <span className="context-usage__key">{t("chat.usage.contextLabel")}</span>
            <span className="context-usage__val">
              {t("chat.usage.percent", { percent: usage.percent })}
            </span>
          </span>
          <span className="context-usage__line">
            <span className="context-usage__key">{t("chat.usage.costLabel")}</span>
            <span className="context-usage__val">{cost}</span>
          </span>
        </span>
      </button>
      <Dialog
        open={open}
        onOpenChange={setOpen}
        titleKey="chat.usage.title"
        size="narrow"
        placement="center"
        surfaceStyle={{ width: "min(380px, calc(100vw - var(--space-6)))" }}
      >
        <div className="context-usage-dialog">
          <div className="context-usage-dialog__summary">
            <span className="context-usage-dialog__full">
              {t("chat.usage.percentFull", { percent: usage.percent })}
            </span>
            <span className="context-usage-dialog__tokens">
              {t("chat.usage.tokens", { used: usedLabel, window: windowLabel })}
            </span>
          </div>
          <div
            className="context-usage-dialog__bar"
            role="img"
            aria-label={t("chat.usage.percentFull", { percent: usage.percent })}
          >
            {visibleBuckets.map((item) => (
              <span
                key={item.id}
                className="context-usage-dialog__seg"
                data-cat={item.id}
                style={{ flexGrow: item.tokens }}
              />
            ))}
            <span
              className="context-usage-dialog__seg context-usage-dialog__seg--free"
              style={{
                flexGrow: Math.max(usage.freeTokens, visibleBuckets.length === 0 ? 1 : 0),
              }}
            />
          </div>
          <ul className="context-usage-dialog__list">
            {listBuckets.map((item) => (
              <li key={item.id} className="context-usage-dialog__row">
                <span className="context-usage-dialog__swatch" data-cat={item.id} />
                <span className="context-usage-dialog__name">
                  {t(`chat.usage.categories.${item.id}`)}
                </span>
                <span className="context-usage-dialog__amount">
                  {formatUsageTokens(item.tokens)}
                </span>
              </li>
            ))}
          </ul>
        </div>
      </Dialog>
    </>
  );
};
