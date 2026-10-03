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

const ATTACHMENT_EXTENSIONS = new Set(
  Object.values(KIND_EXTENSIONS).flatMap((items) => items.map((item) => item.toLowerCase())),
);

const extensionOf = (path: string): string => {
  const base = path.split(/[/\\]/).pop() ?? "";
  const dot = base.lastIndexOf(".");
  if (dot <= 0) return "";
  return base.slice(dot + 1).toLowerCase();
};

const isAttachmentPath = (path: string): boolean => ATTACHMENT_EXTENSIONS.has(extensionOf(path));

const fileUriToPath = (uri: string): string => {
  let rest = uri.trim().replace(/^file:\/\//i, "");
  rest = rest.replace(/^localhost/i, "");
  if (/^\/[A-Za-z]:[\\/]/.test(rest)) rest = rest.slice(1);
  if (!rest.startsWith("/") && !/^[A-Za-z]:[\\/]/.test(rest)) rest = `/${rest}`;
  try {
    return decodeURIComponent(rest);
  } catch {
    return rest;
  }
};

const pathFromLine = (line: string): string | null => {
  const trimmed = line.trim();
  if (!trimmed || trimmed.startsWith("#")) return null;
  if (trimmed === "copy" || trimmed === "cut") return null;
  if (/^file:/i.test(trimmed)) return fileUriToPath(trimmed);
  if (trimmed.startsWith("/") || /^[A-Za-z]:[\\/]/.test(trimmed)) return trimmed;
  return null;
};

const uniquePaths = (paths: string[]): string[] => {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const path of paths) {
    if (seen.has(path)) continue;
    seen.add(path);
    out.push(path);
  }
  return out;
};

const pathsFromBlock = (raw: string): string[] => {
  const out: string[] = [];
  for (const line of raw.split(/\r?\n/)) {
    const path = pathFromLine(line);
    if (path && isAttachmentPath(path)) out.push(path);
  }
  return out;
};

export const clipboardLocalPaths = (event: { clipboardData: DataTransfer | null }): string[] => {
  const data = event.clipboardData;
  if (!data) return [];
  const typed: string[] = [];
  for (const type of ["text/uri-list", "x-special/gnome-copied-files"]) {
    if (!Array.from(data.types ?? []).includes(type)) continue;
    typed.push(...pathsFromBlock(data.getData(type)));
  }
  if (typed.length > 0) return uniquePaths(typed).slice(0, MAX_CHAT_ATTACHMENTS);
  const plain = data.getData("text/plain") ?? "";
  const lines = plain
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  if (lines.length === 0 || lines.length > MAX_CHAT_ATTACHMENTS) return [];
  const paths: string[] = [];
  for (const line of lines) {
    const path = pathFromLine(line);
    if (!path || !isAttachmentPath(path)) return [];
    paths.push(path);
  }
  return uniquePaths(paths);
};

const singleUrl = (value: string): boolean => /^https?:\/\/\S+$/i.test(value);

export const clipboardMayHoldImage = (event: { clipboardData: DataTransfer | null }): boolean => {
  const data = event.clipboardData;
  const types = Array.from(data?.types ?? []);
  if (types.length === 0) return true;
  if (types.some((item) => item === "Files" || item.startsWith("image/"))) return true;
  const plain = (data?.getData("text/plain") ?? "").trim();
  if (types.includes("text/html") && (plain.length === 0 || singleUrl(plain))) return true;
  return false;
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
  const file = (await readTauriClipboardImage()) ?? (await readBrowserClipboardImage());
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
