// Checks that every palette in every theme meets level AA of the Web Content Accessibility
// Guidelines: 4.5:1 for text, 3:1 for the focus outline. It reads the colors straight from
// src/app.css, working out which rules apply the way the browser's cascade would, so a new
// palette or a changed color can't ship without passing. Run it with `pnpm test`.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { PALETTES, DEFAULT_PALETTE } from "../src/lib/palettes.ts";

const css = readFileSync(new URL("../src/app.css", import.meta.url), "utf8");
const html = readFileSync(new URL("../src/app.html", import.meta.url), "utf8");

const TEXT = 4.5;
const NON_TEXT = 3;

// --- Reading the stylesheet --------------------------------------------------------------------

/** Every rule as { media, selector, declarations, order }, with @media blocks flattened. */
function parseRules(source) {
  const rules = [];
  const text = source.replace(/\/\*[\s\S]*?\*\//g, "");
  let order = 0;

  function walk(body, media) {
    let index = 0;
    while (index < body.length) {
      const open = body.indexOf("{", index);
      if (open === -1) break;
      const prelude = body.slice(index, open).trim();
      let depth = 1;
      let close = open + 1;
      while (depth > 0 && close < body.length) {
        if (body[close] === "{") depth++;
        else if (body[close] === "}") depth--;
        close++;
      }
      const inner = body.slice(open + 1, close - 1);
      if (prelude.startsWith("@media")) {
        walk(inner, prelude.slice("@media".length).trim());
      } else {
        const declarations = {};
        for (const part of inner.split(";")) {
          const colon = part.indexOf(":");
          if (colon === -1) continue;
          declarations[part.slice(0, colon).trim()] = part.slice(colon + 1).trim();
        }
        for (const selector of prelude.split(",")) {
          rules.push({ media, selector: selector.trim(), declarations, order: order++ });
        }
      }
      index = close;
    }
  }

  walk(text, null);
  return rules;
}

const ATTRIBUTE = /\[([a-z-]+)(\^?=)"([^"]*)"\]/g;

/**
 * Matches the few selector shapes app.css uses on the root: `:root`, attribute selectors
 * (`[data-x="y"]`, `[data-x^="y"]`) and `:not([...])`. Anything else doesn't match, which
 * leaves out rules for other elements and for the popover window (`:root.popover`).
 * Returns the selector's specificity, or null when it doesn't match.
 */
function match(selector, attributes) {
  if (!selector.startsWith(":root")) return null;
  let rest = selector.slice(":root".length);
  let specificity = 1;

  const test = (name, operator, value) => {
    const actual = attributes[name.replace(/^data-/, "")];
    if (actual === undefined) return false;
    return operator === "=" ? actual === value : actual.startsWith(value);
  };

  rest = rest.replace(/:not\((\[[^\]]+\])\)/g, (_, inner) => {
    const [, name, operator, value] = new RegExp(ATTRIBUTE.source).exec(inner);
    specificity++;
    if (test(name, operator, value)) specificity = -Infinity;
    return "";
  });
  rest = rest.replace(ATTRIBUTE, (_, name, operator, value) => {
    specificity++;
    if (!test(name, operator, value)) specificity = -Infinity;
    return "";
  });

  return rest.trim() === "" && specificity > 0 ? specificity : null;
}

function mediaMatches(media, systemScheme) {
  if (media === null) return true;
  if (media === "(prefers-color-scheme: light)") return systemScheme === "light";
  if (media === "(prefers-color-scheme: dark)") return systemScheme === "dark";
  return false; // reduced motion and anything else: not color, not relevant here
}

const rules = parseRules(css);

/** The custom properties on the root element, after the cascade. */
function tokensFor({ palette, theme, system }) {
  const attributes = {};
  if (palette !== DEFAULT_PALETTE) attributes.palette = palette;
  if (theme !== "system") attributes.theme = theme;

  const matching = rules
    .filter((rule) => mediaMatches(rule.media, system))
    .map((rule) => ({ ...rule, specificity: match(rule.selector, attributes) }))
    .filter((rule) => rule.specificity !== null)
    .sort((a, b) => a.specificity - b.specificity || a.order - b.order);

  const tokens = {};
  for (const rule of matching) {
    for (const [name, value] of Object.entries(rule.declarations)) {
      if (name.startsWith("--")) tokens[name] = value;
    }
  }
  return tokens;
}

// --- Color math (WCAG 2.2 relative luminance and contrast ratio) -------------------------------

function channels(hex) {
  assert.match(hex, /^#[0-9a-f]{6}$/i, `expected a six-digit hex color, got ${hex}`);
  return [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16));
}

function luminance(rgb) {
  const [r, g, b] = rgb.map((value) => {
    const channel = value / 255;
    return channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a, b) {
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (light + 0.05) / (dark + 0.05);
}

/** `color-mix(in srgb, accent <strength>, transparent)` painted over `base`. */
function tint(accent, strength, base) {
  const amount = parseFloat(strength) / 100;
  return accent.map((value, index) => value * amount + base[index] * (1 - amount));
}

// --- The checks --------------------------------------------------------------------------------

const STATES = [
  { theme: "system", system: "dark", scheme: "dark" },
  { theme: "system", system: "light", scheme: "light" },
  { theme: "dark", system: "light", scheme: "dark" },
  { theme: "light", system: "dark", scheme: "light" },
];

for (const { id: palette } of PALETTES) {
  for (const { theme, system, scheme } of STATES) {
    test(`${palette}, theme ${theme}, system ${system}: meets level AA`, () => {
      const tokens = tokensFor({ palette, theme, system });
      const color = (name) => {
        assert.ok(tokens[name], `${name} isn't set`);
        return channels(tokens[name]);
      };

      assert.equal(tokens["color-scheme"] ?? scheme, scheme);

      const pairs = [];
      for (const background of ["--bg", "--surface", "--elevated"]) {
        pairs.push(["--text", background, TEXT], ["--muted", background, TEXT]);
      }
      pairs.push(["--on-accent", "--accent", TEXT]);
      pairs.push(["--focus", "--bg", NON_TEXT], ["--focus", "--surface", NON_TEXT]);

      const failures = [];
      for (const [foreground, background, minimum] of pairs) {
        const ratio = contrast(color(foreground), color(background));
        if (ratio < minimum) {
          failures.push(`${foreground} on ${background}: ${ratio.toFixed(2)}:1, needs ${minimum}:1`);
        }
      }

      // A playing pad's name sits on the accent tint over the pad's background.
      assert.ok(tokens["--playing-strength"], "--playing-strength isn't set");
      const fill = tint(color("--accent"), tokens["--playing-strength"], color("--surface"));
      const onFill = contrast(color("--text"), fill);
      if (onFill < TEXT) {
        failures.push(`--text on the playing fill: ${onFill.toFixed(2)}:1, needs ${TEXT}:1`);
      }

      assert.deepEqual(failures, []);
    });
  }

  test(`${palette}: following the system gives the same colors as choosing the theme`, () => {
    for (const scheme of ["light", "dark"]) {
      assert.deepEqual(
        tokensFor({ palette, theme: "system", system: scheme }),
        tokensFor({ palette, theme: scheme, system: scheme === "light" ? "dark" : "light" }),
        `the ${scheme} colors differ between following the system and choosing ${scheme}`,
      );
    }
  });

  if (palette !== DEFAULT_PALETTE) {
    test(`${palette}: has its own colors and is applied before the first paint`, () => {
      for (const scheme of ["light", "dark"]) {
        assert.notDeepEqual(
          tokensFor({ palette, theme: scheme, system: scheme }),
          tokensFor({ palette: DEFAULT_PALETTE, theme: scheme, system: scheme }),
          `${palette} has no ${scheme} colors of its own in app.css`,
        );
      }
      const pattern = /\/\^catppuccin-\(([^)]+)\)\$\//.exec(html);
      assert.ok(pattern, "app.html's palette check wasn't found");
      assert.ok(
        new RegExp(`^catppuccin-(${pattern[1]})$`).test(palette),
        `app.html doesn't apply ${palette} before the first paint`,
      );
    });
  }
}
