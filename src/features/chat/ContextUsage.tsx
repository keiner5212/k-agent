import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Dialog } from "@/components/Dialog";

const USAGE_CATEGORIES = [
  { id: "systemPrompt", tokens: 1_100 },
  { id: "toolDefinitions", tokens: 9_600 },
  { id: "rules", tokens: 4_400 },
  { id: "skills", tokens: 4_800 },
  { id: "mcpTools", tokens: 622 },
  { id: "subagentDefinitions", tokens: 863 },
  { id: "conversation", tokens: 62_500 },
] as const;

const WINDOW_TOKENS = 256_000;
const DISPLAY_USED_TOKENS = 84_000;
const USED_PERCENT = 33;
const COST_USD = 1.24;
const RING_SIZE = 14;
const RING_STROKE = 2;
const RING_RADIUS = (RING_SIZE - RING_STROKE) / 2;
const RING_CENTER = RING_SIZE / 2;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

const CATEGORY_USED = USAGE_CATEGORIES.reduce((sum, item) => sum + item.tokens, 0);

const formatCost = (usd: number): string =>
  new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" }).format(usd);

const formatTokenCount = (tokens: number): string => {
  if (tokens < 1_000) return String(tokens);
  const k = tokens / 1_000;
  const digits = Number.isInteger(k) && k >= 100 ? 0 : 1;
  return `${k.toFixed(digits)}K`;
};

export const ContextUsage = (): ReactNode => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const cost = formatCost(COST_USD);
  const usedLabel = formatTokenCount(DISPLAY_USED_TOKENS);
  const windowLabel = formatTokenCount(WINDOW_TOKENS);
  const offset = RING_CIRCUMFERENCE * (1 - Math.min(100, Math.max(0, USED_PERCENT)) / 100);
  const freeTokens = Math.max(0, WINDOW_TOKENS - CATEGORY_USED);

  return (
    <>
      <button
        type="button"
        className="context-usage"
        onClick={() => setOpen(true)}
        aria-label={t("chat.usage.label", { percent: USED_PERCENT, cost })}
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
              {t("chat.usage.percent", { percent: USED_PERCENT })}
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
              {t("chat.usage.percentFull", { percent: USED_PERCENT })}
            </span>
            <span className="context-usage-dialog__tokens">
              {t("chat.usage.tokens", { used: usedLabel, window: windowLabel })}
            </span>
          </div>
          <div
            className="context-usage-dialog__bar"
            role="img"
            aria-label={t("chat.usage.percentFull", { percent: USED_PERCENT })}
          >
            {USAGE_CATEGORIES.map((item) => (
              <span
                key={item.id}
                className="context-usage-dialog__seg"
                data-cat={item.id}
                style={{ flexGrow: item.tokens }}
              />
            ))}
            <span
              className="context-usage-dialog__seg context-usage-dialog__seg--free"
              style={{ flexGrow: freeTokens }}
            />
          </div>
          <ul className="context-usage-dialog__list">
            {USAGE_CATEGORIES.map((item) => (
              <li key={item.id} className="context-usage-dialog__row">
                <span className="context-usage-dialog__swatch" data-cat={item.id} />
                <span className="context-usage-dialog__name">
                  {t(`chat.usage.categories.${item.id}`)}
                </span>
                <span className="context-usage-dialog__amount">
                  {formatTokenCount(item.tokens)}
                </span>
              </li>
            ))}
          </ul>
        </div>
      </Dialog>
    </>
  );
};
