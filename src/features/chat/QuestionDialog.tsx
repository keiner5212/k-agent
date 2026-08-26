import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Check, ChevronLeft, ChevronRight, Send, X } from "lucide-react";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { useAskUserStore } from "@/lib/ask-user";
import type { AskUserAnswerEntry, AskUserQuestion, PendingQuestionState } from "@/types/chat";

type QuestionDialogProps = {
  state: PendingQuestionState;
};

const freeTextTrimmed = (text: string): string => text.trim();

const canAdvance = (question: AskUserQuestion, entry: AskUserAnswerEntry | undefined): boolean => {
  if (entry?.skipped) return true;
  if (entry && entry.selected.length > 0) return true;
  if (entry && freeTextTrimmed(entry.freeText).length > 0 && question.allowFreeText !== false)
    return true;
  return false;
};

const isAnswered = (question: AskUserQuestion, entry: AskUserAnswerEntry | undefined): boolean => {
  if (!entry) return false;
  if (entry.selected.length > 0) return true;
  if (question.allowFreeText !== false && freeTextTrimmed(entry.freeText).length > 0) return true;
  return false;
};

export const QuestionDialog = ({ state }: QuestionDialogProps): ReactNode => {
  const { t } = useTranslation();
  const setAnswer = useAskUserStore((store) => store.setAnswer);
  const submit = useAskUserStore((store) => store.submit);
  const cancel = useAskUserStore((store) => store.cancel);
  const [activeIndex, setActiveIndex] = useState(0);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const active = state.questions[activeIndex];
  const activeEntry = state.answers.find((entry) => entry.questionId === active?.id);
  const freeTextRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (state.submitted) return;
    const textarea = freeTextRef.current;
    if (textarea) {
      const length = textarea.value.length;
      textarea.setSelectionRange(length, length);
      textarea.focus();
    }
  }, [activeIndex, state.submitted]);

  const allAnswered = useMemo(
    () =>
      state.questions.every((question) =>
        isAnswered(
          question,
          state.answers.find((entry) => entry.questionId === question.id),
        ),
      ),
    [state.questions, state.answers],
  );

  const handleToggleOption = useCallback(
    (option: string) => {
      if (!active || state.submitted) return;
      const multiSelect = active.multiSelect ?? false;
      const currentSelected = activeEntry?.selected ?? [];
      const exists = currentSelected.includes(option);
      const nextSelected = multiSelect
        ? exists
          ? currentSelected.filter((item) => item !== option)
          : [...currentSelected, option]
        : exists
          ? []
          : [option];
      const nextEntry: AskUserAnswerEntry = {
        questionId: active.id,
        selected: nextSelected,
        freeText: activeEntry?.freeText ?? "",
      };
      setAnswer(state.callId, nextEntry);
    },
    [active, activeEntry, setAnswer, state.callId, state.submitted],
  );

  const handleFreeText = useCallback(
    (event: React.ChangeEvent<HTMLTextAreaElement>) => {
      if (!active) return;
      const nextEntry: AskUserAnswerEntry = {
        questionId: active.id,
        selected: activeEntry?.selected ?? [],
        freeText: event.target.value,
      };
      setAnswer(state.callId, nextEntry);
    },
    [active, activeEntry, setAnswer, state.callId],
  );

  const handleAdvance = useCallback(() => {
    setActiveIndex((current) => Math.min(current + 1, state.questions.length - 1));
  }, [state.questions.length]);

  const handleBack = useCallback(() => {
    setActiveIndex((current) => Math.max(current - 1, 0));
  }, []);

  const handleSubmit = useCallback(async () => {
    if (submitting) return;
    setSubmitting(true);
    setError(undefined);
    const result = await submit(state.callId);
    setSubmitting(false);
    if (result.error) {
      setError(result.error);
    }
  }, [state.callId, submit, submitting]);

  const handleCancel = useCallback(async () => {
    if (state.submitted) return;
    await cancel(state.callId);
  }, [cancel, state.callId, state.submitted]);

  if (!active) return null;
  const multiSelect = active.multiSelect ?? false;
  const allowFreeText = active.allowFreeText !== false;
  const isLast = activeIndex === state.questions.length - 1;
  const showTabs = state.questions.length > 1;

  return (
    <div className="question-dialog" aria-live="polite">
      <header className="question-dialog__header">
        <h4 className="question-dialog__title">{t("chat.question.title")}</h4>
        <IconButton
          label={t("common.off")}
          onClick={() => {
            void handleCancel();
          }}
          disabled={state.submitted}
        >
          <X size={14} strokeWidth={1.5} />
        </IconButton>
      </header>
      {showTabs ? (
        <div className="question-dialog__tabs" role="tablist">
          {state.questions.map((question, index) => {
            const entry = state.answers.find((item) => item.questionId === question.id);
            const answered = isAnswered(question, entry);
            return (
              <button
                key={question.id}
                type="button"
                role="tab"
                aria-selected={index === activeIndex}
                className={`question-dialog__tab${index === activeIndex ? " question-dialog__tab--active" : ""}${answered ? " question-dialog__tab--done" : ""}`}
                onClick={() => setActiveIndex(index)}
              >
                <span className="question-dialog__tab-index">{index + 1}</span>
                <span className="question-dialog__tab-label">{question.header}</span>
              </button>
            );
          })}
        </div>
      ) : null}
      <div className="question-dialog__body" role="tabpanel">
        <p className="question-dialog__question">{active.question}</p>
        <ul
          className={`question-dialog__options${multiSelect ? " question-dialog__options--multi" : ""}`}
          role={multiSelect ? "group" : "radiogroup"}
        >
          {active.options.map((option) => {
            const checked = (activeEntry?.selected ?? []).includes(option.label);
            return (
              <li key={option.label} className="question-dialog__option">
                <button
                  type="button"
                  className={`question-dialog__option-button${checked ? " question-dialog__option-button--checked" : ""}`}
                  aria-pressed={checked}
                  onClick={() => handleToggleOption(option.label)}
                  disabled={state.submitted}
                >
                  <span className="question-dialog__option-marker" aria-hidden="true">
                    {checked ? <Check size={12} strokeWidth={2.5} /> : null}
                  </span>
                  <span className="question-dialog__option-body">
                    <span className="question-dialog__option-label">{option.label}</span>
                    {option.description ? (
                      <span className="question-dialog__option-description">
                        {option.description}
                      </span>
                    ) : null}
                    {option.preview ? (
                      <pre className="question-dialog__option-preview">{option.preview}</pre>
                    ) : null}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
        {allowFreeText ? (
          <label className="question-dialog__free-text">
            <span className="question-dialog__free-text-label">
              {t("chat.question.freeTextLabel")}
            </span>
            <textarea
              ref={freeTextRef}
              className="question-dialog__free-text-input"
              rows={3}
              value={activeEntry?.freeText ?? ""}
              onChange={handleFreeText}
              placeholder={t("chat.question.freeTextPlaceholder")}
              disabled={state.submitted}
            />
          </label>
        ) : null}
      </div>
      {error ? <p className="question-dialog__error">{error}</p> : null}
      <footer className="question-dialog__footer">
        {showTabs ? (
          <GlassButton
            variant="secondary"
            onClick={handleBack}
            disabled={activeIndex === 0 || state.submitted}
          >
            <ChevronLeft size={14} strokeWidth={2} />
            {t("chat.question.back")}
          </GlassButton>
        ) : (
          <span />
        )}
        {isLast ? (
          <GlassButton
            variant="primary"
            onClick={() => {
              void handleSubmit();
            }}
            disabled={state.submitted || submitting || !allAnswered}
          >
            <Send size={14} strokeWidth={2} />
            {submitting ? t("chat.question.submitting") : t("chat.question.submit")}
          </GlassButton>
        ) : (
          <GlassButton
            variant="primary"
            onClick={handleAdvance}
            disabled={state.submitted || !canAdvance(active, activeEntry)}
          >
            {t("chat.question.next")}
            <ChevronRight size={14} strokeWidth={2} />
          </GlassButton>
        )}
      </footer>
    </div>
  );
};
