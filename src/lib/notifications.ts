import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import i18n from "@/i18n";
import { isTauri } from "@/lib/platform";
import { useSettingsStore } from "@/lib/settings";

const BODY_MAX_CHARS = 140;

const stripForBody = (text: string): string => text.replace(/\s+/g, " ").trim();

const buildBody = (content: string): string => {
  const cleaned = stripForBody(content);
  if (cleaned.length === 0) return i18n.t("notifications.responseFinished.empty");
  if (cleaned.length <= BODY_MAX_CHARS) return cleaned;
  return `${cleaned.slice(0, BODY_MAX_CHARS - 1)}...`;
};

const windowIsFocused = (): boolean => {
  if (typeof document === "undefined") return false;
  if (document.visibilityState !== "visible") return false;
  return document.hasFocus();
};

let permissionRequested = false;

export const ensurePermission = async (): Promise<boolean> => {
  if (!isTauri()) return false;
  try {
    if (await isPermissionGranted()) return true;
    if (permissionRequested) return false;
    permissionRequested = true;
    const result = await requestPermission();
    return result === "granted";
  } catch (error) {
    console.warn("notification permission check failed", error);
    return false;
  }
};

export const notifyResponseFinished = async (content: string): Promise<void> => {
  if (!useSettingsStore.getState().notificationsEnabled) return;
  if (windowIsFocused()) return;
  const allowed = await ensurePermission();
  if (!allowed) return;
  try {
    sendNotification({
      title: i18n.t("notifications.responseFinished.title"),
      body: buildBody(content),
    });
  } catch (error) {
    console.warn("notification send failed", error);
  }
};

export const notifyAskUser = async (count: number, firstQuestion: string | null): Promise<void> => {
  if (!useSettingsStore.getState().notificationsEnabled) return;
  if (windowIsFocused()) return;
  const allowed = await ensurePermission();
  if (!allowed) return;
  const title = i18n.t("notifications.askUser.title");
  const body =
    count === 1 && firstQuestion
      ? i18n.t("notifications.askUser.single", { question: firstQuestion })
      : i18n.t("notifications.askUser.multiple", { count });
  try {
    sendNotification({ title, body });
  } catch (error) {
    console.warn("notification send failed", error);
  }
};
