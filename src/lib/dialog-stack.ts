type DialogCloseHandler = () => void;

const stack: DialogCloseHandler[] = [];

export const pushDialog = (close: DialogCloseHandler): void => {
  stack.push(close);
};

export const popDialog = (close: DialogCloseHandler): void => {
  const index = stack.lastIndexOf(close);
  if (index >= 0) stack.splice(index, 1);
};

export const hasOpenDialogs = (): boolean => stack.length > 0;

export const closeTopDialog = (): boolean => {
  const top = stack[stack.length - 1];
  if (!top) return false;
  top();
  return true;
};

export const closeAllDialogs = (): void => {
  const handlers = [...stack];
  stack.length = 0;
  for (let index = handlers.length - 1; index >= 0; index -= 1) {
    handlers[index]();
  }
};
