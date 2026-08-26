import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { LineKind } from "@/components/LineEditor";
import { ReadOnlyEditorDialog } from "@/features/chat/ReadOnlyEditorDialog";
import { estimateTokensFromText } from "@/lib/jobs-handlers";
import { diffEditorValue, readSessionFileRevision, yamlBlockValue } from "@/lib/session-files";
import { skillNameFromCall, type ChatToolCall } from "@/types/chat";
import { formatContextWindow } from "@/types/providers";

type ToolCallsBlockProps = {
  calls: ChatToolCall[];
  sessionId: string | null;
};

type PreviewState = {
  titleKey: string;
  value: string;
  path?: string;
  startLine?: number;
  lineNumbers?: number[];
  lineKinds?: LineKind[];
};

const fileName = (path: string): string => {
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || path;
};

const previewFromOutput = (call: ChatToolCall): string => {
  if (call.name === "skill")
    return yamlBlockValue(call.output ?? "", "body") || (call.output ?? "");
  if (call.name === "read")
    return yamlBlockValue(call.output ?? "", "content") || (call.output ?? "");
  if (call.name === "list_directory") {
    return yamlBlockValue(call.output ?? "", "entries") || (call.output ?? "");
  }
  return call.output ?? "";
};

const toolCallLabel = (call: ChatToolCall, lineRange = ""): string => {
  const name = call.name;
  if (name === "skill") {
    const skill = skillNameFromCall(call);
    return skill ? `${name} "${skill}"` : name;
  }
  const path = call.display?.path?.trim() || call.argument?.trim() || "";
  if (name === "read") {
    if (!path) return name;
    return lineRange ? `${name} "${path}" ${lineRange}` : `${name} "${path}"`;
  }
  if (name === "list_directory") return path ? `${name} "${path}"` : name;
  if (call.display?.kind === "action") {
    return path ? `${name} ${fileName(path)}` : name;
  }
  if (path) return `${name} "${path}"`;
  return name;
};

const ToolCallsBlock = ({ calls, sessionId }: ToolCallsBlockProps): ReactNode => {
  const { t } = useTranslation();
  const [preview, setPreview] = useState<PreviewState | null>(null);
  if (calls.length === 0) return null;

  const openPreview = async (call: ChatToolCall): Promise<void> => {
    const display = call.display;
    if (display?.kind === "action" && call.id && sessionId) {
      if (call.name === "create_folder" || call.name === "delete") {
        setPreview({
          titleKey:
            call.name === "delete" ? "chat.tools.deleteTitle" : "chat.tools.createFolderTitle",
          value: call.output ?? "",
          path: display.path,
        });
        return;
      }
      if (display.status !== "ok") {
        setPreview({
          titleKey: call.name === "edit" ? "chat.tools.editTitle" : "chat.tools.writeTitle",
          value: display.path ? `${display.path}\n${t("chat.tools.error")}` : t("chat.tools.error"),
          path: display.path,
        });
        return;
      }
      try {
        const after = await readSessionFileRevision(sessionId, call.id, "after");
        if (call.name === "write") {
          setPreview({
            titleKey: "chat.tools.writeTitle",
            value: after.content,
            path: display.path,
            startLine: 1,
          });
          return;
        }
        const before = await readSessionFileRevision(sessionId, call.id, "before");
        const packed = diffEditorValue(before.content, after.content);
        setPreview({
          titleKey: "chat.tools.editTitle",
          value: packed.value,
          path: display.path,
          lineNumbers: packed.lineNumbers,
          lineKinds: packed.lineKinds,
        });
        return;
      } catch {
        setPreview({
          titleKey: "chat.tools.outputTitle",
          value: call.output ?? "",
          path: display.path,
        });
        return;
      }
    }
    const titleKey =
      call.name === "skill"
        ? "chat.tools.skillTitle"
        : call.name === "read"
          ? "chat.tools.readTitle"
          : call.name === "list_directory"
            ? "chat.tools.listTitle"
            : "chat.tools.outputTitle";
    setPreview({
      titleKey,
      value: previewFromOutput(call),
      path: display?.path,
      startLine: call.name === "read" ? display?.startLine : undefined,
    });
  };

  return (
    <>
      <ul className="chat-tools">
        {calls.map((call, index) => {
          const display = call.display;
          const output = call.output?.trim() ?? "";
          const tokens = output.length > 0 ? estimateTokensFromText(output) : null;
          const isAction = display?.kind === "action";
          const lineRange =
            display?.startLine !== undefined && display.endLine !== undefined
              ? t("chat.tools.lines", { start: display.startLine, end: display.endLine })
              : "";
          const label = toolCallLabel(call, lineRange);
          const canOpen = Boolean(call.output) || Boolean(isAction && call.id);
          return (
            <li
              key={call.id ?? `${call.name}-${index}`}
              className={`chat-tools__item${call.name === "skill" ? " chat-tools__item--skill" : ""}${isAction ? " chat-tools__item--action" : ""}`}
            >
              <span className="chat-tools__line">
                {canOpen ? (
                  <button
                    type="button"
                    className="chat-tools__name"
                    onClick={() => {
                      void openPreview(call);
                    }}
                  >
                    {label}
                  </button>
                ) : (
                  <span>{label}</span>
                )}
                {isAction ? (
                  <span
                    className={`chat-tools__status${display?.status === "ok" ? " chat-tools__status--ok" : " chat-tools__status--error"}`}
                  >
                    {display?.status === "ok" ? t("chat.tools.ok") : t("chat.tools.error")}
                  </span>
                ) : null}
                {isAction && display?.added !== undefined && display.removed !== undefined ? (
                  <span className="chat-tools__diff">
                    <span className="change-bar__added">+{display.added}</span>
                    <span className="change-bar__removed">-{display.removed}</span>
                  </span>
                ) : null}
                {!isAction && tokens !== null ? (
                  <span className="chat-tools__tokens">~{formatContextWindow(tokens)}</span>
                ) : null}
              </span>
            </li>
          );
        })}
      </ul>
      <ReadOnlyEditorDialog
        open={preview !== null}
        titleKey={preview?.titleKey ?? "chat.tools.outputTitle"}
        value={preview?.value ?? ""}
        path={preview?.path}
        startLine={preview?.startLine}
        lineNumbers={preview?.lineNumbers}
        lineKinds={preview?.lineKinds}
        onOpenChange={(open) => {
          if (!open) setPreview(null);
        }}
      />
    </>
  );
};

export { ToolCallsBlock };
export type { ToolCallsBlockProps };
