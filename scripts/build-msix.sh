#!/usr/bin/env bash
# Packs a release's portable Windows executables into a Microsoft Store package: one .msix for
# each architecture and an .msixbundle of them. Run on a Windows machine with the Windows SDK
# (makeappx) and the GitHub CLI; msix.yml does it on a runner.
#
#   scripts/build-msix.sh <tag> <identity name> <publisher> <publisher display name> [output folder]
#
# The three identity values are Partner Center's (Product identity), copied exactly: a package
# whose identity differs from the reserved name is rejected. The package is left unsigned on
# purpose, because the Store signs it.
set -euo pipefail

if [[ $# -lt 4 ]]; then
  sed -n '2,10p' "$0"
  exit 2
fi
tag="$1"
identity_name="$2"
publisher="$3"
publisher_display_name="$4"
output="${5:-msix-out}"

version="${tag#v}"
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "The tag must look like v1.2.3, got: $tag" >&2
  exit 1
fi
# The Store wants four numbers and keeps the last one, the revision, for itself: it must be 0.
package_version="$version.0"

makeappx="$(find "/c/Program Files (x86)/Windows Kits/10/bin" -path '*/x64/makeappx.exe' 2>/dev/null | sort -V | tail -n 1)"
if [[ -z "$makeappx" ]]; then
  echo "makeappx.exe not found: install the Windows SDK." >&2
  exit 1
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$output" "$work/bundle"

# XML would break on these in an attribute; Partner Center's values never have them.
for value in "$identity_name" "$publisher" "$publisher_display_name"; do
  if [[ "$value" == *'&'* || "$value" == *'<'* || "$value" == *'|'* ]]; then
    echo "An identity value has a character this script can't place in the manifest: $value" >&2
    exit 1
  fi
done

for architecture in x64 x86 arm64; do
  layout="$work/$architecture"
  mkdir -p "$layout/Assets"
  gh release download "$tag" --repo itriumid/honk --pattern "Honk_${version}_${architecture}-portable.exe" --output "$layout/Honk.exe"
  for logo in StoreLogo Square44x44Logo Square150x150Logo; do
    cp "$root/src-tauri/icons/$logo.png" "$layout/Assets/$logo.png"
  done
  sed \
    -e "s|@@IDENTITY_NAME@@|$identity_name|" \
    -e "s|@@PUBLISHER@@|$publisher|" \
    -e "s|@@PUBLISHER_DISPLAY_NAME@@|$publisher_display_name|" \
    -e "s|@@VERSION@@|$package_version|" \
    -e "s|@@ARCHITECTURE@@|$architecture|" \
    "$root/msix/AppxManifest.xml" > "$layout/AppxManifest.xml"
  "$makeappx" pack /o /d "$(cygpath -w "$layout")" /p "$(cygpath -w "$work/bundle/Honk_${package_version}_${architecture}.msix")"
done

"$makeappx" bundle /o /bv "$package_version" /d "$(cygpath -w "$work/bundle")" /p "$(cygpath -w "$PWD/$output/Honk_${package_version}.msixbundle")"
echo "Wrote $output/Honk_${package_version}.msixbundle"
