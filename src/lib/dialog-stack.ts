type DialogCloseHandler = () => void;

type DialogEntry = {
  id: string;
  close: DialogCloseHandler;
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

export const pushDialog = (close: DialogCloseHandler, id: string): void => {
  stack.push({ id, close });
  emit();
};

export const popDialog = (close: DialogCloseHandler): void => {
  let index = -1;
  for (let cursor = stack.length - 1; cursor >= 0; cursor -= 1) {
    if (stack[cursor]?.close === close) {
      index = cursor;
      break;
    }
  }
  if (index >= 0) stack.splice(index, 1);
  emit();
};

export const topDialogId = (): string | null => stack[stack.length - 1]?.id ?? null;

export const hasOpenDialogs = (): boolean => stack.length > 0;

export const closeTopDialog = (): boolean => {
  const top = stack[stack.length - 1];
  if (!top) return false;
  top.close();
  return true;
};

export const closeAllDialogs = (): void => {
  const handlers = stack.map((entry) => entry.close);
  stack.length = 0;
  emit();
  for (let index = handlers.length - 1; index >= 0; index -= 1) {
    handlers[index]();
  }
};
