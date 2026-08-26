import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { readImage } from "@tauri-apps/plugin-clipboard-manager";
import type { AttachmentKind, ChatAttachment } from "@/types/chat";
import type { ModelInfo } from "@/types/providers";
import { isTauri } from "@/lib/platform";
import { readSessionAttachment } from "@/lib/session-files";

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

const pushUniqueFile = (files: File[], seen: Set<string>, file: File | null): void => {
  if (!file) return;
  const key = `${file.name}:${file.size}:${file.type}:${file.lastModified}`;
  if (seen.has(key)) return;
  seen.add(key);
  files.push(file);
};

export const collectClipboardFiles = (event: { clipboardData: DataTransfer | null }): File[] => {
  const data = event.clipboardData;
  if (!data) return [];
  const files: File[] = [];
  const seen = new Set<string>();
  const items = data.items;
  if (items) {
    for (let index = 0; index < items.length; index += 1) {
      const item = items[index];
      if (!item || item.kind !== "file") continue;
      pushUniqueFile(files, seen, item.getAsFile());
    }
  }
  const list = data.files;
  if (list) {
    for (let index = 0; index < list.length; index += 1) {
      pushUniqueFile(files, seen, list.item(index));
    }
  }
  return files;
};

export const clipboardLooksLikeAttachment = (event: {
  clipboardData: DataTransfer | null;
}): boolean => {
  const data = event.clipboardData;
  const types = Array.from(data?.types ?? []);
  if (types.some((item) => item === "Files" || item.startsWith("image/"))) return true;
  if (types.length === 0) return true;
  if (types.some((item) => item.startsWith("text/html"))) return false;
  return (data?.getData("text/plain") ?? "").trim().length === 0;
};

const rgbaToPngFile = (rgba: Uint8Array, width: number, height: number): Promise<File | null> =>
  new Promise((resolve) => {
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      resolve(null);
      return;
    }
    const imageData = ctx.createImageData(width, height);
    imageData.data.set(rgba);
    ctx.putImageData(imageData, 0, 0);
    canvas.toBlob((blob) => {
      if (!blob) {
        resolve(null);
        return;
      }
      resolve(new File([blob], "clipboard.png", { type: "image/png" }));
    }, "image/png");
  });

const readBrowserClipboardImage = async (): Promise<File | null> => {
  if (!navigator.clipboard || typeof navigator.clipboard.read !== "function") return null;
  try {
    const items = await navigator.clipboard.read();
    for (const item of items) {
      const type = item.types.find((value) => value.startsWith("image/"));
      if (!type) continue;
      const blob = await item.getType(type);
      const subtype = type.split("/")[1] ?? "png";
      const ext = subtype === "jpeg" ? "jpg" : subtype;
      return new File([blob], `clipboard.${ext}`, { type });
    }
  } catch {
    return null;
  }
  return null;
};

const readTauriClipboardImage = async (): Promise<File | null> => {
  if (!isTauri()) return null;
  try {
    const image = await readImage();
    const rgba = await image.rgba();
    const size = await image.size();
    const file = await rgbaToPngFile(rgba, size.width, size.height);
    const closer = image as { close?: () => Promise<void> };
    await closer.close?.();
    return file;
  } catch {
    return null;
  }
};

export const prepareFileAttachments = async (
  files: File[],
  allowedTypes: AttachmentKind[],
): Promise<PrepareAttachmentsResult> => {
  if (files.length === 0) return { attachments: [], errors: [] };
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

export const prepareClipboardImageAttachments = async (
  allowedTypes: AttachmentKind[],
): Promise<PrepareAttachmentsResult> => {
  const file = (await readBrowserClipboardImage()) ?? (await readTauriClipboardImage());
  if (!file) return { attachments: [], errors: [] };
  return prepareFileAttachments([file], allowedTypes);
};

export const blobFromAttachment = (item: ChatAttachment): Blob | null => {
  if (!item.data) return null;
  const binary = atob(item.data);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return new Blob([bytes], { type: item.mime || "application/octet-stream" });
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

export const useHydratedAttachment = (
  sessionId: string | null,
  item: ChatAttachment,
): ChatAttachment => {
  const [fetched, setFetched] = useState<ChatAttachment | null>(null);
  useEffect(() => {
    if (item.data || item.kind === "text" || item.kind === "document" || !sessionId) return;
    let cancelled = false;
    void readSessionAttachment(sessionId, item.id)
      .then((next) => {
        if (!cancelled) setFetched(next);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [item.data, item.file, item.id, item.kind, sessionId]);
  if (item.data || item.kind === "text" || item.kind === "document") return item;
  if (fetched && fetched.id === item.id && fetched.data) return fetched;
  return item;
};
