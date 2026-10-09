# The Microsoft Store package

Honk goes to the Microsoft Store as an MSIX package. The Store signs it for us and hosts the
download, so people get no "unknown publisher" warning from SmartScreen, and we need no code
signing certificate for this channel.

Tauri doesn't build MSIX itself. `scripts/build-msix.sh` packs the release's portable
executables (x64, x86 and arm64) into one `.msixbundle`, and the *Microsoft Store package*
workflow runs it. Nothing here changes how releases are built.

## One-time setup (Partner Center)

1. Register as an individual developer at <https://storedeveloper.microsoft.com> (no fee, and
   identity verification with a government ID). Registering from the older Partner Center page
   starts the old flow.
2. Reserve the name **Honk**.
3. In the reserved product's **Product identity** page, copy three values exactly:
   `Package/Identity/Name`, `Package/Identity/Publisher` (`CN=…`) and
   `Package/Properties/PublisherDisplayName`. A package whose identity differs is rejected.

## For each release

1. Publish the release as usual.
2. Run the workflow: Actions → *Microsoft Store package* → Run workflow, with the tag and the
   three values above. It keeps `honk-microsoft-store-package` as an artifact.
3. In Partner Center, create a submission, upload the `.msixbundle`, and fill in the listing
   (description, screenshots, age rating, privacy policy link). Certification takes a few days.

`runFullTrust` is the one capability the package declares, as every desktop application does;
the submission asks why, and the answer is that Honk is a desktop application.

## Not tried yet

The packaging has been written but not run: the first run on a runner may need small fixes, and
the first Store certification may ask for changes to the listing or the manifest.
