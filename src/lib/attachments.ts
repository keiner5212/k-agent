import { invoke } from "@tauri-apps/api/core";
import type { AttachmentKind, ChatAttachment } from "@/types/chat";
import type { ModelInfo } from "@/types/providers";

export const ATTACHMENT_KINDS: readonly AttachmentKind[] = [
  "image",
  "pdf",
  "video",
  "audio",
  "text",
  "document",
] as const;

const KIND_EXTENSIONS: Record<AttachmentKind, readonly string[]> = {
  image: ["png", "jpg", "jpeg", "gif", "webp"],
  pdf: ["pdf"],
  video: ["mp4", "webm", "mov", "mpeg", "mpg", "avi", "wmv", "3gp"],
  audio: ["mp3", "wav", "m4a", "aac", "webm"],
  text: ["txt", "md"],
  document: ["docx"],
};

const PICKER_KINDS: readonly AttachmentKind[] = ["image", "pdf", "video", "text", "document"];

export const MAX_CHAT_ATTACHMENTS = 8;

export type AttachmentErrorCode =
  "unsupported" | "tooLarge" | "unreadable" | "extractFailed" | "tooMany";

type PrepareAttachmentsResult = {
  attachments: ChatAttachment[];
  errors: { name: string; code: AttachmentErrorCode }[];
};

const asKind = (value: string): AttachmentKind | null => {
  const key = value.trim().toLowerCase();
  return ATTACHMENT_KINDS.find((item) => item === key) ?? null;
};

export const modelAttachmentTypes = (model: ModelInfo | null | undefined): AttachmentKind[] => {
  if (!model) return [];
  const seen = new Set<AttachmentKind>();
  const out: AttachmentKind[] = [];
  for (const value of model.attachmentTypes ?? []) {
    const kind = asKind(value);
    if (!kind || seen.has(kind)) continue;
    seen.add(kind);
    out.push(kind);
  }
  if (out.length > 0) return out;
  for (const value of model.input ?? []) {
    const kind = asKind(value);
    if (!kind || kind === "text" || seen.has(kind)) continue;
    seen.add(kind);
    out.push(kind);
  }
  if (model.attachment) {
    for (const kind of ["text", "document"] as const) {
      if (seen.has(kind)) continue;
      seen.add(kind);
      out.push(kind);
    }
  }
  return out;
};

export const pickerAttachmentTypes = (model: ModelInfo | null | undefined): AttachmentKind[] =>
  modelAttachmentTypes(model).filter((kind) => PICKER_KINDS.includes(kind));

export const dialogFiltersFor = (
  types: AttachmentKind[],
  allLabel: string,
): { name: string; extensions: string[] }[] => {
  const extensions = [...new Set(types.flatMap((kind) => [...KIND_EXTENSIONS[kind]]))];
  if (extensions.length === 0) return [];
  return [{ name: allLabel, extensions }];
};

const fileToBase64 = async (file: File): Promise<string> => {
  const buffer = await file.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
};

const clipboardFiles = (event: { clipboardData: DataTransfer | null }): File[] => {
  const files: File[] = [];
  const list = event.clipboardData?.items;
  if (!list) return files;
  for (const item of list) {
    if (item.kind !== "file") continue;
    const file = item.getAsFile();
    if (file) files.push(file);
  }
  return files;
};

export const clipboardHasFiles = (event: { clipboardData: DataTransfer | null }): boolean =>
  clipboardFiles(event).length > 0;

export const prepareClipboardAttachments = async (
  event: { clipboardData: DataTransfer | null },
  allowedTypes: AttachmentKind[],
): Promise<PrepareAttachmentsResult> => {
  const files = clipboardFiles(event);
  if (files.length === 0) {
    return { attachments: [], errors: [] };
  }
  const blobs = await Promise.all(
    files.map(async (file) => ({
      name: file.name || "clipboard.png",
      mime: file.type || undefined,
      data: await fileToBase64(file),
    })),
  );
  return invoke<PrepareAttachmentsResult>("prepare_chat_attachments", {
    input: { blobs, allowedTypes },
  });
};

export const preparePathAttachments = async (
  paths: string[],
  allowedTypes: AttachmentKind[],
): Promise<PrepareAttachmentsResult> =>
  invoke<PrepareAttachmentsResult>("prepare_chat_attachments", {
    input: { paths, allowedTypes },
  });

export const attachmentPreviewUrl = (item: ChatAttachment): string | null => {
  if (item.kind !== "image" || !item.data) return null;
  return `data:${item.mime};base64,${item.data}`;
};
