# SmartBridge Setup

The official installer for **SmartBridge** — the macOS / Windows
companion app that bridges Cubase, Synthesizer V, and a Yamaha
Tyros / Genos workstation.

**[➜ Download the latest version](../../releases/latest)**

---

## Download

Each release ships **three flavors** of SmartBridge Setup. Pick the one
that matches what you were invited to:

| Flavor | What it is | Pick this if… |
|---|---|---|
| **Release** | Full SmartBridge, no time limit, no activation prompt. | …you bought / received a normal license. |
| **Demo** | Full SmartBridge that stops working **30 days after first launch**. | …you want to evaluate before deciding. |
| **Beta 0.1** | Invitation-only build. Asks for the **email + serial** we sent you. | …you got a beta invitation email. |

All three install side-by-side (different bundle identifiers and product
names) — you can have Demo and Release installed at the same time
without conflict if you want to.

Pick the file for your platform from the [Releases](../../releases)
page:

| Platform | Release | Demo | Beta 0.1 |
|---|---|---|---|
| **macOS** (Apple Silicon, macOS 11+) | `SmartBridge_Setup_<v>_arm64.dmg` | `SmartBridge_Setup_Demo_<v>_arm64.dmg` | `SmartBridge_Setup_Beta-0.1_<v>_arm64.dmg` |
| **Windows** (x64, Windows 10 1809+)  | `SmartBridge_Setup_<v>_x64-setup.exe` | `SmartBridge_Setup_Demo_<v>_x64-setup.exe` | `SmartBridge_Setup_Beta-0.1_<v>_x64-setup.exe` |
| **Windows** (MSI, IT deployment)     | `SmartBridge_Setup_<v>_x64.msi` | `SmartBridge_Setup_Demo_<v>_x64.msi` | `SmartBridge_Setup_Beta-0.1_<v>_x64.msi` |

Each download has a `.sha256` sidecar so you can verify integrity.

### What the three flavors actually do

- **Release** – the Setup app installs SmartBridge and writes a tiny
  `license.json` saying "no checks, ever". You will never see a
  countdown banner or an activation dialog.

- **Demo** – the Setup app does not write any license file. The first
  time SmartBridge launches, it stamps "today" into its own
  `license.json` and starts a 30-day countdown. After 30 days, the
  app shows an "expired" message and quits. There is no online
  check; the timer lives in the local file. Reinstalling the app
  does not reset the timer (the file outlives the app), but
  manually deleting `~/Library/SmartBridge/license.json` (or the
  equivalent on Windows) does. We're aware. It's a soft gate, not
  copy-protection.

- **Beta 0.1** – the Setup app shows an **Activate** dialog before
  it will install anything. Enter your email address and the 16-
  character serial we sent in your invitation
  (`XXXX-XXXX-XXXX-XXXX`, case + dashes don't matter). Setup
  validates the serial locally (no internet round-trip) and writes
  it to `license.json`. The plugin re-validates the same pair on
  every launch.

  If you fat-finger the serial, the dialog stays open and lets you
  retry — Setup does not exit until you either succeed or click
  **Quit**.

---

## What is SmartBridge Setup?

A small (~3 MB) bootstrapper that detects what's already on your
machine and installs only the components you actually want:

- **SmartBridge** — the main app, plugin, and database.
- **Cubase connection** — MIDI Remote driver script and project template.
- **Synthesizer V connection** — side-panel script for SynthV Studio 1 / 2.
- **AI Lyrics** — local lyric-generation model (uses Ollama if installed).
- **SmartBridge resources** — seed configuration on first install.
- **Help files** — getting-started guide and multilingual manual.

Each component shows its current status (Ready / Not installed /
Needs repair) on the dashboard. You install, repair, or skip
individually — no all-or-nothing setup.

---

## Install

### macOS

1. Download the `.dmg`.
2. Open it, drag `SmartBridge Setup.app` to Applications.
3. The first launch may show a Gatekeeper warning — right-click the
   app, choose **Open**, then **Open** in the dialog. (Pre-1.0
   builds may not be notarized yet; this is being addressed.)
4. The dashboard appears. Click **Install** on each component you
   want.

### Windows

1. Download the `_x64-setup.exe`.
2. Double-click to run. Approve the SmartScreen / UAC prompts
   (signed builds will not show SmartScreen warnings; pre-1.0
   builds may).
3. Follow the wizard.
4. Launch **SmartBridge Setup** from the Start Menu and click
   **Install** on each component.

For unattended IT deployment, use the `.msi` package and standard
`msiexec /i ... /qn` flags.

---

## Verifying your download

Each release ships a `<filename>.sha256` next to its installer.

**macOS:**

```bash
shasum -a 256 -c SmartBridge_Setup_<version>_arm64.dmg.sha256
```

**Windows (PowerShell):**

```powershell
$expected = (Get-Content .\SmartBridge_Setup_<version>_x64-setup.exe.sha256).Split(' ')[0]
$actual   = (Get-FileHash .\SmartBridge_Setup_<version>_x64-setup.exe -Algorithm SHA256).Hash.ToLower()
if ($expected -eq $actual) { "OK" } else { "MISMATCH" }
```

---

## What this repo contains

Just the published Setup installers, nothing else. The "Source code"
zip and tar.gz that GitHub adds automatically to each release page
contain only this README — they are auto-generated by the GitHub
release system and can be ignored.

The Setup app itself, once running, downloads the actual SmartBridge
components from a separate **asset feed** repo at
[`smartbridge-releases`](https://github.com/claudioreyesdeveloper/smartbridge-releases).
End users do not need to visit that repo — the Setup app handles
everything in the background, with SHA256 verification on every
download.

---

## Offline / air-gapped install

If you don't want SmartBridge Setup to touch the internet — for
privacy, restricted networks, IT-managed deployments, or just
because you'd rather keep an archived copy locally — you can install
entirely from a single pre-downloaded bundle.

### 1. Get the offline bundle

Each SmartBridge release on the asset-feed repo has a single self-
contained zip you can grab manually:

```
smartbridge-offline-bundle-<version>.zip
```

Find it under
[smartbridge-releases / Releases](https://github.com/claudioreyesdeveloper/smartbridge-releases/releases)
on the release page that matches the SmartBridge version you want
to install. The zip contains the manifest plus every component
(macOS pkg, Windows installer, Cubase script and template, SynthV
script, help files, seed configuration, etc.) along with a
`README.txt` and SHA256 checksums.

### 2. Unzip it anywhere

Pick a folder you'll keep — a USB stick, a network share, a folder
on your Desktop, anything. The bundle is a single directory.

### 3. Tell SmartBridge Setup to use it

1. Launch SmartBridge Setup.
2. Click the **Diagnostics** tab.
3. Find the section **Offline / local repository**.
4. Paste the absolute path to the unzipped bundle folder and click
   **Use this folder**.
5. An **Offline** badge appears next to the version in the header.
6. Switch back to the **Dashboard**. From this point on, every
   install action reads files from your local folder instead of the
   internet. SHA256 verification still runs against the manifest.

To go back to online mode, click **Disable offline mode** in the
same Diagnostics section.

### Scripted / IT deployment

You can also lock the offline path with an environment variable so
end users can't change it from the UI:

```bash
# macOS / Linux
export SMARTBRIDGE_LOCAL_REPO=/path/to/smartbridge-offline-bundle-<version>
```

```powershell
# Windows (per-user, persistent)
[Environment]::SetEnvironmentVariable(
  "SMARTBRIDGE_LOCAL_REPO",
  "C:\Path\to\smartbridge-offline-bundle-<version>",
  "User"
)
```

When the env var is set, the Diagnostics input is locked and the
status reads "set via SMARTBRIDGE_LOCAL_REPO env var".

### What's in the bundle

| File | Purpose |
|---|---|
| `smartbridge-release-manifest.json` | Single source of truth: components, files, SHA256s |
| `SmartBridge_<v>.pkg` | macOS plugin installer |
| `SmartBridge_<v>_Setup.exe` | Windows plugin installer |
| `SmartBridge.cpr` | Cubase project template |
| `SmartBridge_GenosSlotRename.js` | Cubase MIDI Remote driver |
| `synthv_smartbridge_sidepanel.lua` | Synthesizer V side-panel script |
| `loopMIDISetup_<v>.zip` | Windows-only MIDI loopback runtime (Tobias Erichsen) |
| `config-default.json` | Seed configuration (no secrets) |
| `Installation_guide.zip` | Getting-started PDF and one-pager |
| `smartbridge_multilingual_manual.zip` | Full multilingual manual |
| `build_features.{macos,windows}.json` | Per-platform feature flags |
| `README.txt` | Plain-text instructions |

Bundle sizes typically run 350–400 MB depending on the release.

---

## Source code

The SmartBridge source code is in the private
[`SmartBridge-Plugin`](https://github.com/claudioreyesdeveloper/SmartBridge-Plugin)
repository. Contact the author for licensing or integration
questions.

---

## Versioning

Setup releases are tagged `setup-v<MAJOR.MINOR.PATCH>` (e.g.
`setup-v0.1.0`, `setup-v2.0.0`). Pre-1.0 releases are flagged as
**Pre-release** until general availability. Each tag publishes the
**Release**, **Demo**, and **Beta 0.1** flavors at once.

---

## Lost or wrong Beta serial?

Reply to your beta-invitation email with the address you typed into
Setup. Serials are derived deterministically from the (lowercased,
trimmed) email; the same email always produces the same serial, so
we can re-send it without rotating anything else.

---

## License

The SmartBridge Setup application is © 2026 Claudio Reyes. Binary
redistribution outside this repository is not permitted.

The SmartBridge plugin and its bundled assets are distributed under
the SmartBridge end-user license, included with each install.
