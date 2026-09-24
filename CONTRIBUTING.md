# Contributing to Honk

Thanks for helping. This page covers setting up, the rules a pull request has to pass, and how
releases are cut. The rules are short, and two of them are enforced by required checks, so
reading them first saves you a red pull request.

## Reporting a bug or asking for a feature

Open a [GitHub Issue](https://github.com/itriumid/honk/issues). Search first, since the
known platform limitations are already filed. For a bug, include your OS and version, how you
installed Honk (`.dmg`, `.exe`, `.msi`, `.deb`, `.rpm` or `.AppImage`), and the steps to
reproduce it. Tech debt and tooling problems have their own **Technical debt** form.

## Setting up

You need [Rust](https://rustup.rs) (stable), Node.js 24 and pnpm 12.

```sh
pnpm install
pnpm tauri dev
```

On Linux, install Tauri's [system dependencies](https://tauri.app/start/prerequisites/#linux)
plus the ALSA headers (`libasound2-dev` on Debian and Ubuntu, `alsa-lib-devel` on Fedora).

Before opening a pull request, run all three checks, even if your change only looks like it
touches one side:

| What | Command |
| --- | --- |
| Type-check the frontend | `pnpm check` |
| Build the frontend | `pnpm build` |
| Test the Rust side | `cargo test` (from `src-tauri/`) |

The README's **Architecture** and **Design** sections are the plan and the design tokens. Two
rules there are easy to break by accident: audio plays in Rust, never in the webview, and
colours come from the tokens in `src/app.css`, never a hard-coded hex value.

## Pull requests

Honk follows a shared set of conventions, kept in
[agent-handbook](https://github.com/itriumid/agent-handbook/blob/main/conventions). The parts that matter here:

- **Branch from `main`** and name the branch `<type>/[<issue>-]<slug>`, where `<type>` is one of
  `feature`, `enhancement`, `fix` or `chore`: for example `fix/12-hotkey-crash` or
  `feature/pad-reordering`. The **Branch name** check enforces it.
  [Details and how to pick a type](https://github.com/itriumid/agent-handbook/blob/main/conventions/rules/branching.md).
- **Title the pull request in the imperative mood**, with no `feat:`-style prefix or trailing
  period ("Add pad reordering"). It becomes the merge commit's subject, so it's the changelog.
- **Fill in the template**: Why, What changed, How to verify. Write `Closes #12` if it resolves
  an issue. [More on descriptions and commits](https://github.com/itriumid/agent-handbook/blob/main/conventions/rules/pull-requests.md).
- **Commits are kept as written.** Pull requests land as merge commits, never squashed, so
  make each commit one reviewable change with a real message. To catch up with `main`,
  rebase, don't merge it in.
- **No tool attribution** in commits or the pull request: no `Co-Authored-By:` bot trailers, no
  "generated with". The **Commit messages** check enforces it. Using tools is fine; stamping
  them into history isn't. [Why](https://github.com/itriumid/agent-handbook/blob/main/conventions/rules/ai-agents.md#no-self-attribution-in-anything-kept).
  If you work with an AI coding tool, point it at [`AGENTS.md`](AGENTS.md).

Every pull request also has to pass **Build (macOS)**, **Build (Linux)** and **Build
(Windows)**. They type-check, test and bundle the app on each OS through
[`build.yml`](.github/workflows/build.yml). They also run on every push to `main`, which is
what keeps the build cache warm for pull requests.

## Versioning

Honk follows [Semantic Versioning](https://semver.org): `MAJOR.MINOR.PATCH`, tagged `v0.2.0`.
Honk is an app, not a library, so "compatible" means what people rely on between versions:

- their library: sounds, settings, hotkeys and categories
- `.honk` files they've exported or been sent
- the operating systems Honk runs on

| Bump | When | Example |
| --- | --- | --- |
| **MAJOR** | Something people rely on stops working: an older library or `.honk` file no longer opens, a feature is removed, or an OS version is no longer supported | `1.4.2` → `2.0.0` |
| **MINOR** | Anything new or improved, still compatible | `1.4.2` → `1.5.0` |
| **PATCH** | Bug fixes only | `1.4.2` → `1.4.3` |

The pull request labels since the last release decide it: any `type: feature` or
`type: enhancement` means at least a minor bump, and only `type: bug` means a patch. A breaking
change says so in its pull request description.

While Honk is `0.x`, the major number stays at 0: a breaking change bumps **minor** instead, and
everything else bumps **patch**. `1.0.0` is the first version with a promise of stability.

Some things hold at every version, whatever the number says:

- **Upgrading never loses a library.** Database changes are migrations that only add, so any
  version opens a library made by any earlier one.
- **No prerelease or build suffixes** such as `-beta.1` or `+build.5`. Windows installers only
  accept numeric versions, so the release workflow refuses those tags. To try a release before
  tagging it, use the dry run described below.

## Releasing

Maintainers only.

1. Pick the version (see **Versioning** above), then set it everywhere it's recorded:

   ```sh
   pnpm bump-version 0.2.0
   ```

   That updates `package.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` and
   `src-tauri/tauri.conf.json`. It refuses anything that isn't `MAJOR.MINOR.PATCH`, or isn't
   higher than the current version. Land the four files through a pull request, like any other
   change.

2. Tag the merge commit on `main` and push the tag. Release tags are annotated, so they record
   who tagged them and when, and so they also work when git is set to sign tags
   (`tag.gpgSign`), which needs a message:

   ```sh
   git switch main && git pull
   git tag -a v0.2.0 -m "Honk 0.2.0"
   git push origin v0.2.0
   ```

3. [`release.yml`](.github/workflows/release.yml) checks that the tag is `vMAJOR.MINOR.PATCH` and
   matches the app's version, creates a **draft** release with notes generated from the merged
   pull requests' labels, and builds the installers on each operating system, attaching them to
   that draft as they finish.
4. When all three builds are green, open the draft on the Releases page, check the notes and the
   attached files, and click **Publish release**.

To try the release builds without making a release, run the **Release** workflow manually from
the Actions tab. It builds the same installers and keeps them as workflow artifacts instead.
