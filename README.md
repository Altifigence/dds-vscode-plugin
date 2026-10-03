# DDS integration for Visual Studio Code

The open-source integration behind the **VS Code** entry in Digital Design
Studio's Plugins catalog. It registers DDS's device-wide integration setting,
detects a separately installed VS Code application, and opens the current DDS
project in a new VS Code window. On Windows, a WSL project uses its exact
distribution and Linux folder after checking the separately installed Microsoft
WSL extension.

This repository includes real DDS integration source and an independent adapter
build. `dds-source/` contains six byte-preserved DDS source files, including the
Tauri command wrappers. `src/` builds the actual TypeScript host adapter and
contracts with local import paths. `native/` compiles the selected Rust policy
functions unchanged and supplies public project/settings ports for embedding.
The full DDS desktop host, project ownership, authentication, UI and settings
persistence remain responsibilities of the embedding application. This is a DDS
integration source package; it is not a VS Code VSIX extension.

## Use in DDS

1. Install Digital Design Studio and [Visual Studio Code](https://code.visualstudio.com/download) separately.
2. For Windows WSL projects, install [Microsoft's WSL extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-wsl) in VS Code separately.
3. In DDS's Plugins catalog, enable the VS Code integration.
4. Open a local DDS project and use the VS Code action. DDS opens the current
   native folder, or the selected WSL distribution and Linux folder, in a new
   window. The published native adapter reports Cloud launches as unsupported.

Enabling this integration changes DDS registration. It does not install VS Code
or an extension. Registration and application availability are separate states.
Disabling the integration removes its DDS registration without uninstalling
external software.

## Build and test from source

Requirements: Node.js 22.23.1 or newer and Rust 1.88.0 with Cargo. No DDS checkout,
private registry, service credentials, VS Code application or WSL distribution
is needed for these checks.

`npm pack` includes the built JavaScript, its TypeScript and native Rust sources,
test fixtures, licenses and source inventory. Build caches and executable
artifacts are excluded.

```sh
git clone https://github.com/Altifigence/dds-vscode-plugin.git
cd dds-vscode-plugin
npm ci
npm test
cargo test --manifest-path native/Cargo.toml --locked --all-targets
cargo clippy --manifest-path native/Cargo.toml --locked --all-targets -- -D warnings
```

Node tests exercise the exported DDS adapter, strict IPC validation, registration
version checks, and scoped invalidation. Rust tests exercise the original policy
functions, registration ports, WSL identity/CLI validation, environment filtering,
rate limits, and bounded process handling. Process tests launch only this
repository's temporary fixture executable. They do not open projects or use a
user's installed VS Code. GitHub Actions runs the checks on hosted Ubuntu and
Windows runners.

```sh
# Verify snapshot hashes and compiled source selections in this repository.
npm run verify:source

# Optional maintainer check against an authorized local DDS checkout.
node scripts/verify-source.mjs /path/to/DDS
```

`SOURCE_INVENTORY.json` records the DDS source revision, SHA-256 values, exact
inclusive line ranges, and the only TypeScript transformation: replacing private
package import paths with local imports. New standalone host wrappers are in
`native/src/lib.rs`; they are distinguished from the unchanged compiled policy
fragments in `native/src/source/`. The snapshot Tauri wrappers depend on DDS and
are supplied as integration source, rather than being compiled by this standalone
crate. This package does not install a general DDS plugin runtime.

## Embed the adapter

After `npm run build`, import the adapter and connect the native host's invoke
function. The renderer sends a fixed editor ID and registration boolean; project
paths, executable paths and arbitrary arguments never cross this interface.

```js
import { createDesktopVsCodeIntegration } from './dist/index.js';

// invoke is supplied by your authorized DDS native host.
const integration = createDesktopVsCodeIntegration(invoke);
const state = await integration.read();
await integration.setInstalled(true);
// Call from the user's explicit Open action after checking the returned state.
await integration.open();
```

The Rust crate exposes `WorkspaceLease` and `InstallationSettings` traits.
Implement the project lease from the host's immutable, authorized project, and
use a durable device-wide settings store with atomic merge-patch writes.
Registration uses `extensions["altifigence.vscode"]` with `installed: true` and
`version: "1.0.0"`; JSON null removes that entry. Call the blocking native adapter
on a host worker thread. Window labels must come from the native host: `main` can
register/open, and `settings` can read. The host owns these identities and cannot
delegate them to renderer input. These ports do not replace project authorization.

The native adapter discovers VS Code only in its fixed OS installation catalog,
validates executable trust, sanitizes inherited loader and credential variables,
and spawns without a shell. WSL probing inspects a bounded official CLI wrapper
layout and invokes the installed CLI bootstrap directly, with a 10-second timeout
and 16 KiB combined output limit. Unknown layouts and malformed results fail
closed. Unix installations that are symlinks, lack executable bits, have an
untrusted owner or are writable by another account are rejected by the retained
DDS policy.

## License

Altifigence's selected original integration files and source ranges listed in
`SOURCE_INVENTORY.json`, plus the new standalone adapter code, are released under
[Apache-2.0](LICENSE). [NOTICE](NOTICE) makes the selected-file grant explicit,
including the exported DDS console-core files. Other DDS code is outside this
grant.

VS Code's open-source source code is [MIT licensed](https://github.com/microsoft/vscode/blob/main/LICENSE.txt);
Microsoft's distributed VS Code application uses [separate product license terms](https://code.visualstudio.com/License/).
Microsoft extension packages and Marketplace access use their own terms, and
[Microsoft's brand guidelines](https://code.visualstudio.com/brand) govern names
and brand assets. This repository includes no Microsoft source, binary, VSIX,
logo or brand image and does not claim Microsoft endorsement.
