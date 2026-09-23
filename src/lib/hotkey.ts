/**
 * Hotkeys travel as the global-shortcut plugin's strings — `shift+control+alt+super+KeyA` —
 * built from the key's physical `code`, so Option changing the typed character doesn't matter.
 */

const MODIFIER_CODES = new Set([
  "ShiftLeft",
  "ShiftRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
  "CapsLock",
  "Fn",
]);

export type RecordedKey =
  | { kind: "hotkey"; hotkey: string }
  | { kind: "cancel" }
  /** A modifier on its own, or a key with no modifier: keep listening. */
  | { kind: "incomplete" };

export function fromKeyboardEvent(event: KeyboardEvent): RecordedKey {
  const modifiers = [
    event.shiftKey && "shift",
    event.ctrlKey && "control",
    event.altKey && "alt",
    event.metaKey && "super",
  ].filter(Boolean);

  if (event.code === "Escape" && modifiers.length === 0) return { kind: "cancel" };
  if (MODIFIER_CODES.has(event.code) || !event.code) return { kind: "incomplete" };
  // The backend enforces this too; checking here keeps the recorder listening instead.
  if (!event.ctrlKey && !event.altKey && !event.metaKey) return { kind: "incomplete" };

  return { kind: "hotkey", hotkey: [...modifiers, event.code].join("+") };
}

const SYMBOLS: Record<string, string> = {
  control: "⌃",
  alt: "⌥",
  shift: "⇧",
  super: "⌘",
};

/** Apple's order for modifier symbols. */
const SYMBOL_ORDER = ["control", "alt", "shift", "super"];

const KEY_NAMES: Record<string, string> = {
  Escape: "⎋",
  Enter: "↩",
  Space: "Space",
  Tab: "⇥",
  Backspace: "⌫",
  Delete: "⌦",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backquote: "`",
};

function keyName(code: string): string {
  if (code in KEY_NAMES) return KEY_NAMES[code];
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("Numpad")) return `Num ${code.slice(6)}`;
  return code;
}

/** `alt+shift+KeyA` → `⌥⇧A`. */
export function formatHotkey(hotkey: string): string {
  const parts = hotkey.split("+");
  const key = parts.pop() ?? "";
  const modifiers = SYMBOL_ORDER.filter((modifier) => parts.includes(modifier));
  return modifiers.map((modifier) => SYMBOLS[modifier]).join("") + keyName(key);
}
