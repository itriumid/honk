// Sets Honk's version everywhere it's recorded, so the four copies can't drift apart:
//
//   pnpm bump-version 0.2.0
//
// Versions are MAJOR.MINOR.PATCH only (see CONTRIBUTING.md, "Versioning"), and must be higher
// than the current one.
import { readFileSync, writeFileSync } from "node:fs";

const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

const next = process.argv[2]?.replace(/^v/, "");
if (!next || !SEMVER.test(next)) {
  fail(
    `usage: pnpm bump-version MAJOR.MINOR.PATCH, e.g. 0.2.0` +
      (next ? `\n"${next}" isn't one. Prerelease and build suffixes aren't supported.` : ""),
  );
}

const packageJson = JSON.parse(readFileSync("package.json", "utf8"));
const current = packageJson.version;
if (compare(next, current) <= 0) fail(`${next} isn't higher than the current version, ${current}`);

updateJson("package.json", (data) => (data.version = next));
updateJson("src-tauri/tauri.conf.json", (data) => (data.version = next));
// The first `version` in Cargo.toml is the one under [package].
replaceOnce("src-tauri/Cargo.toml", /^version = "[^"]*"$/m, `version = "${next}"`);
// Cargo.lock records the app's own version too; CI builds with --locked, so it must match.
replaceOnce(
  "src-tauri/Cargo.lock",
  /(\[\[package\]\]\nname = "honk"\nversion = ")[^"]*(")/,
  `$1${next}$2`,
);

console.log(`${current} → ${next}. Commit the four files, then follow "Releasing" in CONTRIBUTING.md.`);

function compare(left, right) {
  const a = left.split(".").map(Number);
  const b = right.split(".").map(Number);
  for (let index = 0; index < 3; index++) {
    if (a[index] !== b[index]) return a[index] - b[index];
  }
  return 0;
}

function updateJson(path, change) {
  const data = JSON.parse(readFileSync(path, "utf8"));
  change(data);
  writeFileSync(path, JSON.stringify(data, null, 2) + "\n");
}

function replaceOnce(path, pattern, replacement) {
  const text = readFileSync(path, "utf8");
  if (!pattern.test(text)) fail(`couldn't find the version in ${path}`);
  writeFileSync(path, text.replace(pattern, replacement));
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
