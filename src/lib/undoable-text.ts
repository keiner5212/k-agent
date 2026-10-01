import { useCallback, useEffect, useRef } from "react";

const MAX_HISTORY = 200;

export const useUndoableText = (
  value: string,
  onChange: (next: string) => void,
): {
  pushChange: (next: string) => void;
  undo: () => void;
  redo: () => void;
} => {
  const historyRef = useRef<string[]>([value]);
  const historyIndexRef = useRef(0);
  const lastEmittedRef = useRef(value);
  const onChangeRef = useRef(onChange);

  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  useEffect(() => {
    if (value === lastEmittedRef.current) return;
    historyRef.current = [value];
    historyIndexRef.current = 0;
    lastEmittedRef.current = value;
  }, [value]);

  const emitChange = (next: string): void => {
    lastEmittedRef.current = next;
    onChangeRef.current(next);
  };

  const pushChange = (next: string): void => {
    const current = historyRef.current[historyIndexRef.current];
    if (current === next) return;
    const stack = historyRef.current.slice(0, historyIndexRef.current + 1);
    stack.push(next);
    if (stack.length > MAX_HISTORY) stack.shift();
    historyRef.current = stack;
    historyIndexRef.current = stack.length - 1;
    emitChange(next);
  };

  const undo = useCallback((): void => {
    if (historyIndexRef.current <= 0) return;
    historyIndexRef.current -= 1;
    const next = historyRef.current[historyIndexRef.current] ?? "";
    lastEmittedRef.current = next;
    onChangeRef.current(next);
  }, []);

  const redo = useCallback((): void => {
    if (historyIndexRef.current >= historyRef.current.length - 1) return;
    historyIndexRef.current += 1;
    const next = historyRef.current[historyIndexRef.current] ?? "";
    lastEmittedRef.current = next;
    onChangeRef.current(next);
  }, []);

  return { pushChange, undo, redo };
};
