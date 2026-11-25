type DialogCloseHandler = () => void;

type DialogEntry = {
  id: string;
  close: DialogCloseHandler;
  closing: boolean;
};

const stack: DialogEntry[] = [];
const listeners = new Set<() => void>();

const emit = (): void => {
  for (const listener of listeners) listener();
};

export const subscribeDialogs = (listener: () => void): (() => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

// Same id updates close in place. Pop-then-push would move a covered dialog above the one on screen.
export const bindDialog = (id: string, close: DialogCloseHandler): void => {
  const existing = stack.find((entry) => entry.id === id);
  if (existing) {
    existing.close = close;
    return;
  }
  stack.push({ id, close, closing: false });
  emit();
};

export const releaseDialog = (id: string): void => {
  const index = stack.findIndex((entry) => entry.id === id);
  if (index < 0) return;
  stack.splice(index, 1);
  emit();
};

export const markDialogClosing = (id: string): void => {
  const entry = stack.find((item) => item.id === id);
  if (!entry || entry.closing) return;
  entry.closing = true;
};

export const topDialogId = (): string | null => stack[stack.length - 1]?.id ?? null;

export const hasOpenDialogs = (): boolean => stack.some((entry) => !entry.closing);

export const closeTopDialog = (): boolean => {
  for (let index = stack.length - 1; index >= 0; index -= 1) {
    const entry = stack[index];
    if (!entry || entry.closing) continue;
    entry.closing = true;
    entry.close();
    return true;
  }
  return false;
};

export const closeAllDialogs = (): void => {
  const handlers: DialogCloseHandler[] = [];
  for (let index = stack.length - 1; index >= 0; index -= 1) {
    const entry = stack[index];
    if (!entry || entry.closing) continue;
    entry.closing = true;
    handlers.push(entry.close);
  }
  for (const close of handlers) close();
};
