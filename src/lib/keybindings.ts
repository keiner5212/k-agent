import { isMac } from "./platform";

export type ChordKey = {
  key: string;
  code: string;
  meta: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
};

const KEY_ALIASES: Record<string, string> = {
  " ": "Space",
  esc: "Escape",
  escape: "Escape",
  arrowup: "ArrowUp",
  arrowdown: "ArrowDown",
  arrowleft: "ArrowLeft",
  arrowright: "ArrowRight",
  enter: "Enter",
  tab: "Tab",
};

export const eventToChord = (event: KeyboardEvent): ChordKey => ({
  key: (KEY_ALIASES[event.key.toLowerCase()] ?? event.key).toLowerCase(),
  code: event.code,
  meta: event.metaKey,
  ctrl: event.ctrlKey,
  alt: event.altKey,
  shift: event.shiftKey,
});

export const chordToString = (chord: ChordKey): string => {
  const parts: string[] = [];
  if (chord.meta) parts.push(isMac() ? "Cmd" : "Meta");
  if (chord.ctrl) parts.push(isMac() ? "Ctrl" : "Ctrl");
  if (chord.alt) parts.push("Alt");
  if (chord.shift) parts.push("Shift");
  const key = chord.key === " " ? "Space" : chord.key;
  parts.push(key.length === 1 ? key.toUpperCase() : key);
  return parts.join("+");
};

export const chordEquals = (a: ChordKey, b: ChordKey): boolean =>
  a.key === b.key &&
  a.meta === b.meta &&
  a.ctrl === b.ctrl &&
  a.alt === b.alt &&
  a.shift === b.shift;

export const stringToChord = (raw: string): ChordKey | null => {
  const tokens = raw
    .split("+")
    .map((token) => token.trim())
    .filter(Boolean);
  if (tokens.length === 0) return null;

  const chord: ChordKey = {
    key: "",
    code: "",
    meta: false,
    ctrl: false,
    alt: false,
    shift: false,
  };

  for (const token of tokens) {
    const lower = token.toLowerCase();
    if (lower === "mod") {
      if (isMac()) chord.meta = true;
      else chord.ctrl = true;
    } else if (lower === "cmd" || lower === "meta" || lower === "super") {
      chord.meta = true;
    } else if (lower === "ctrl" || lower === "control") {
      chord.ctrl = true;
    } else if (lower === "alt" || lower === "option") {
      chord.alt = true;
    } else if (lower === "shift") {
      chord.shift = true;
    } else {
      chord.key = (KEY_ALIASES[lower] ?? token).toLowerCase();
      chord.code = token;
    }
  }

  if (!chord.key) return null;
  return chord;
};

export const parseChord = (event: KeyboardEvent): ChordKey | null => {
  const chord = eventToChord(event);
  if (
    !chord.key ||
    chord.key === "shift" ||
    chord.key === "control" ||
    chord.key === "alt" ||
    chord.key === "meta"
  ) {
    return null;
  }
  return chord;
};

export const matchesChordString = (event: KeyboardEvent, raw: string): boolean => {
  const expected = stringToChord(raw);
  if (!expected) return false;
  const actual = parseChord(event);
  if (!actual) return false;
  return chordEquals(actual, expected);
};

export const formatChordLabel = (raw: string): string =>
  raw
    .split("+")
    .map((token) => {
      const lower = token.trim().toLowerCase();
      if (
        lower === "mod" ||
        lower === "meta" ||
        lower === "cmd" ||
        lower === "command" ||
        lower === "super" ||
        lower === "control"
      ) {
        return "Ctrl";
      }
      return token.trim();
    })
    .filter(Boolean)
    .join("+");

export const EDITOR_SAVE_EVENT = "k-agent:editor-save";

export const isEditableTarget = (target: EventTarget | null): boolean =>
  target instanceof HTMLInputElement ||
  target instanceof HTMLTextAreaElement ||
  target instanceof HTMLSelectElement ||
  (target instanceof HTMLElement && target.isContentEditable);
