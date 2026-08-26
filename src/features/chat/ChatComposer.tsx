import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { ArrowUp, Paperclip, Play } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { GlassButton } from "@/components/GlassButton";
import { IconButton } from "@/components/IconButton";
import { MagicGenerateButton } from "@/components/MagicGenerateButton";
import {
  clipboardLooksLikeAttachment,
  collectClipboardFiles,
  dialogFiltersFor,
  MAX_CHAT_ATTACHMENTS,
  pickerAttachmentTypes,
  prepareClipboardImageAttachments,
  prepareFileAttachments,
  preparePathAttachments,
} from "@/lib/attachments";
import { generateAppContent } from "@/lib/app-generation";
import { useChatStore } from "@/lib/chat";
import { useComposerStore } from "@/lib/composer";
import { resolveSelectedModel } from "@/lib/context-usage";
import { isTauri } from "@/lib/platform";
import { useProvidersStore } from "@/lib/providers";
import { useRewindConfirmStore } from "@/lib/rewind-confirm";
import { matchActionSlashCommand } from "@/lib/slash-commands";
import { useSelectionStore } from "@/lib/selected-model";
import { useSettingsStore } from "@/lib/settings";
import { useSessionsStore } from "@/lib/sessions";
import { useUndoRedoKeydown } from "@/lib/use-undo-redo-keydown";
import { useUndoableText } from "@/lib/undoable-text";
import { AgentSelector } from "./AgentSelector";
import { ComposerAttachments } from "./ComposerAttachments";
import { ComposerShellChip } from "./ComposerModeChip";
import { ComposerQueue } from "./ComposerQueue";
import { ComposerTextarea } from "./ComposerTextarea";
import { ContextUsage } from "./ContextUsage";
import { EffortSelector } from "./EffortSelector";
import { ModelSelector } from "./ModelSelector";
import { useComposerFocusKeys } from "./use-composer-focus";

export const ChatComposer = (): ReactNode => {
  const { t } = useTranslation();
  const value = useComposerStore((state) => state.value);
  const mode = useComposerStore((state) => state.mode);
  const attachments = useComposerStore((state) => state.attachments);
  const setValue = useComposerStore((state) => state.setValue);
  const addAttachments = useComposerStore((state) => state.addAttachments);
  const setAttachments = useComposerStore((state) => state.setAttachments);
  const clearComposer = useComposerStore((state) => state.clear);
  const selection = useSelectionStore((state) => state.selection);
  const providers = useProvidersStore((state) => state.providers);
  const sending = useChatStore((state) => state.sending);
  const shellRunning = useSessionsStore((state) => state.shellRunning);
  const hydrated = useChatStore((state) => state.hydrated);
  const sendMessage = useChatStore((state) => state.send);
  const runShell = useSessionsStore((state) => state.runShell);
  const enqueue = useSessionsStore((state) => state.enqueue);
  const requestRewind = useRewindConfirmStore((state) => state.requestRewind);
  const keybindings = useSettingsStore((state) => state.keybindings);
  const appGenerationModel = useSettingsStore((state) => state.appGenerationModel);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const { pushChange, undo, redo } = useUndoableText(value, setValue);
  const [attachError, setAttachError] = useState<string | undefined>();
  const [improving, setImproving] = useState(false);
  const [improveError, setImproveError] = useState<string | undefined>();
  const model = useMemo(() => resolveSelectedModel(providers, selection), [providers, selection]);
  const allowedTypes = useMemo(() => pickerAttachmentTypes(model), [model]);
  const shellMode = mode === "shell";
  const busy = sending || shellRunning;
  const hasPayload = value.trim().length > 0 || attachments.length > 0;
  const canQueue =
    busy && hydrated && (shellMode ? value.trim().length > 0 : hasPayload && Boolean(selection));
  const canSend = Boolean(selection) && hydrated && hasPayload && !busy;
  const canRunShell = hydrated && value.trim().length > 0 && !busy;
  const canAttach = !shellMode && isTauri() && allowedTypes.length > 0;
  const generationModel = appGenerationModel ?? selection;
  const canImprove =
    !shellMode && isTauri() && Boolean(generationModel) && value.trim().length > 0 && !busy;

  const runSlashAction = useCallback(
    (commandId: string): void => {
      if (commandId === "undo") {
        requestRewind();
      }
    },
    [requestRewind],
  );

  useEffect(() => {
    if (attachments.length === 0) return;
    const allowed = new Set(allowedTypes);
    const next = attachments.filter((item) => allowed.has(item.kind));
    if (next.length === attachments.length) return;
    setAttachments(next);
  }, [allowedTypes, attachments, setAttachments]);

  const ingestPrepared = useCallback(
    async (work: () => ReturnType<typeof preparePathAttachments>): Promise<void> => {
      if (attachments.length >= MAX_CHAT_ATTACHMENTS) {
        setAttachError(t("chat.composer.attachTooMany"));
        return;
      }
      try {
        const result = await work();
        if (result.attachments.length > 0) addAttachments(result.attachments);
        const first = result.errors[0];
        if (first) {
          setAttachError(t(`chat.composer.attachError.${first.code}`, { name: first.name }));
          return;
        }
        setAttachError(undefined);
      } catch {
        setAttachError(t("chat.composer.attachError.unreadable", { name: "" }));
      }
    },
    [addAttachments, attachments.length, t],
  );

  const handleAttach = useCallback(() => {
    if (!canAttach) return;
    void (async () => {
      const picked = await openDialog({
        multiple: true,
        title: t("chat.composer.attach"),
        filters: dialogFiltersFor(allowedTypes, t("chat.composer.attachFilter")),
      });
      const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
      if (paths.length === 0) return;
      await ingestPrepared(() => preparePathAttachments(paths, allowedTypes));
    })();
  }, [allowedTypes, canAttach, ingestPrepared, t]);

  const handlePaste = useCallback(
    (event: ClipboardEvent<HTMLElement>) => {
      if (event.defaultPrevented || shellMode || !canAttach) return;
      const files = collectClipboardFiles(event.nativeEvent);
      if (files.length > 0) {
        event.preventDefault();
        void ingestPrepared(() => prepareFileAttachments(files, allowedTypes));
        return;
      }
      if (!clipboardLooksLikeAttachment(event.nativeEvent)) return;
      event.preventDefault();
      void ingestPrepared(() => prepareClipboardImageAttachments(allowedTypes));
    },
    [allowedTypes, canAttach, ingestPrepared, shellMode],
  );

  const handleSend = useCallback(async () => {
    if (busy) {
      if (!canQueue) return;
      if (!shellMode && matchActionSlashCommand(value)) return;
      enqueue(value, mode, shellMode ? undefined : attachments);
      clearComposer();
      textareaRef.current?.focus();
      return;
    }
    if (shellMode) {
      if (!canRunShell) return;
      const text = value;
      clearComposer();
      await runShell(text);
      textareaRef.current?.focus();
      return;
    }
    if (!canSend) return;
    const text = value;
    const pending = attachments;
    const action = matchActionSlashCommand(text);
    if (action) {
      clearComposer();
      runSlashAction(action.id);
      textareaRef.current?.focus();
      return;
    }
    const accepted = await sendMessage(text, undefined, pending);
    if (accepted) clearComposer();
    textareaRef.current?.focus();
  }, [
    attachments,
    busy,
    canQueue,
    canRunShell,
    canSend,
    clearComposer,
    enqueue,
    mode,
    runShell,
    runSlashAction,
    sendMessage,
    shellMode,
    value,
  ]);

  const handleComposerKeyDown = useCallback(
    (event: KeyboardEvent<HTMLTextAreaElement>) => {
      if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) return;
      if (shellMode) {
        if (!canRunShell && !canQueue) return;
        event.preventDefault();
        void handleSend();
        return;
      }
      if (!canSend && !canQueue) return;
      event.preventDefault();
      void handleSend();
    },
    [canQueue, canRunShell, canSend, handleSend, shellMode],
  );

  const sendEnabled = shellMode ? canRunShell || canQueue : canSend || canQueue;
  const sendLabel = canQueue
    ? t("chat.composer.queue")
    : shellMode
      ? canRunShell
        ? t("chat.composer.run")
        : t("chat.composer.disabledHintShell")
      : canSend
        ? t("chat.composer.send")
        : t("chat.composer.disabledHint");
  const attachLabel = canAttach ? t("chat.composer.attach") : t("chat.composer.attachDisabled");
  const improveLabel = canImprove
    ? t("chat.composer.improve")
    : value.trim().length === 0
      ? t("chat.composer.improveEmpty")
      : t("chat.composer.improveDisabled");

  const handleImprove = useCallback(() => {
    if (!canImprove || improving) return;
    void (async () => {
      setImproving(true);
      setImproveError(undefined);
      const result = await generateAppContent({
        kind: "improvePrompt",
        content: value,
      });
      setImproving(false);
      if (result.error) {
        setImproveError(result.error === "noModel" ? t("appGeneration.noModel") : result.error);
        return;
      }
      if (result.text) pushChange(result.text);
    })();
  }, [canImprove, improving, pushChange, t, value]);

  useUndoRedoKeydown(textareaRef, keybindings, undo, redo);
  useComposerFocusKeys(textareaRef, pushChange);

  return (
    <footer
      className="chat-composer"
      onPaste={handlePaste}
      {...(shellMode ? { "data-mode": "shell" } : {})}
    >
      <div className="chat-composer__toolbar">
        {shellMode ? <ComposerShellChip /> : null}
        {!shellMode ? <AgentSelector /> : null}
        {!shellMode ? <ModelSelector /> : null}
        {!shellMode ? <EffortSelector /> : null}
        <ContextUsage />
      </div>
      <ComposerQueue />
      {!shellMode ? <ComposerAttachments /> : null}
      {attachError ? <p className="chat-composer__attach-error">{attachError}</p> : null}
      {improveError ? <p className="chat-composer__attach-error">{improveError}</p> : null}
      <div className="chat-composer__field">
        <ComposerTextarea
          value={value}
          mode={mode}
          onChange={pushChange}
          textareaRef={textareaRef}
          onKeyDown={handleComposerKeyDown}
          onPaste={handlePaste}
        />
        <div className="chat-composer__actions">
          {!shellMode ? (
            <MagicGenerateButton
              label={improveLabel}
              className="chat-composer__magic"
              onClick={handleImprove}
              disabled={!canImprove}
              loading={improving}
            />
          ) : null}
          {!shellMode ? (
            <IconButton
              label={attachLabel}
              className="chat-composer__attach"
              onClick={handleAttach}
              disabled={!canAttach}
            >
              <Paperclip size={16} strokeWidth={1.5} />
            </IconButton>
          ) : null}
          <GlassButton
            variant="primary"
            className={`chat-composer__send${shellMode ? " chat-composer__send--shell" : ""}`}
            aria-label={sendLabel}
            title={sendLabel}
            onClick={() => {
              void handleSend();
            }}
            disabled={!sendEnabled}
          >
            {shellMode ? <Play size={16} strokeWidth={2} /> : <ArrowUp strokeWidth={2} />}
          </GlassButton>
        </div>
      </div>
    </footer>
  );
};
