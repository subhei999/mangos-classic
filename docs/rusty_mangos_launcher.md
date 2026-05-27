# Rusty MaNGOS Windows Launcher

Windows-first installer and Rust `egui` GUI launcher for the Rust
authserver/worldserver stack.

The intended player experience is:

1. Run the Rusty MaNGOS installer.
2. Open **Rusty MaNGOS Launcher** from the Start Menu or desktop shortcut.
3. Pick the World of Warcraft 1.12.1 client folder.
4. Click **Install / Start**.
5. Log in with the local seeded account.

The command wrappers still exist for development and troubleshooting, but they
are backend automation for the launcher rather than the primary user interface.

## First Run

From an installed package, open:

```text
Rusty MaNGOS Launcher
```

From a source checkout, run:

```powershell
cargo run -p rusty-mangos-launcher
```

The launcher will:

- ask for the World of Warcraft 1.12.1 client folder;
- extract required server-side `dbc`, `maps`, `vmaps`, and starter `mmaps`
  data from that client into `target\launcher\data`;
- download and unpack portable MariaDB under `target\launcher\mariadb`;
- initialize a local MariaDB data directory under `target\launcher\mariadb-data`;
- clone/import ClassicDB into the local `mangos` world database when needed;
- generate launcher config files under `target\launcher`;
- update `realmlist.wtf` to `set realmlist 127.0.0.1:13724`;
- build and start the Rust authserver and worldserver.

No Docker Desktop is required for the normal launcher flow.

Packaged installs use bundled `server\authserver.exe` and
`server\worldserver.exe`, so players do not need Rust/Cargo installed.

## Login

Use the seeded local account:

```text
RUSTAUTH
RUSTPASS
```

## Launcher UI

The launcher is styled like a dark game launcher:

- left navigation rail for **Server**, **Setup**, **Logs**, and **Advanced**;
- large server card with database/auth/world status pills;
- install/start progress panel with current phase, elapsed time, and command
  detail;
- primary **Install / Start** or **Restart** action;
- quick **Start**, **Stop**, **Dashboard**, and app-data shortcuts;
- setup page for choosing or auto-detecting the WoW client folder;
- health page for client data, ports, MariaDB data, and realmlist status;
- logs page with launcher, authserver, worldserver, and MariaDB log tabs;
- repair page for database repair, vmap/mmap rebuilds, world reimport, and
  seeded character reset;
- updates page for checking the rolling GitHub launcher release and downloading
  the latest app zip or installer into launcher data;
- advanced page for ports, ClassicDB path, and import/realmlist options.

## Start And Stop

Use the GUI buttons:

- **Install / Start**
- **Configure**
- **Start**
- **Stop**
- **Restart**
- **Status**
- **Dashboard**

Command wrappers are also available:

```powershell
.\scripts\rusty-mangos-launcher.cmd Start
.\scripts\rusty-mangos-launcher.cmd Stop
.\scripts\rusty-mangos-launcher.cmd Status
.\scripts\rusty-mangos-launcher.cmd CheckUpdates
```

The first install also writes convenience wrappers into `target\launcher`:

- `Start Rusty MaNGOS.cmd`
- `Stop Rusty MaNGOS.cmd`
- `Restart Rusty MaNGOS.cmd`
- `Rusty MaNGOS Status.cmd`
- `Configure Rusty MaNGOS.cmd`

## Building The Installer

From a source checkout:

```powershell
.\scripts\package-rusty-mangos-launcher.ps1
```

This builds:

- release `authserver.exe` and `worldserver.exe`;
- native Rust `egui` `RustyMangosLauncher.exe`;
- a distributable app folder under `target\launcher-package\app`;
- `RustyMangosSetup.exe`.

If Inno Setup's compiler is not already on `PATH`, the packaging script
downloads the official Inno Setup installer and installs the compiler into
`target\tooling\inno-setup`. This is build-machine tooling only; players only
run `RustyMangosSetup.exe`.

## Antivirus Reputation

Unsigned game launchers are frequently flagged by reputation-based scanners,
especially when they start hidden helper processes, download dependencies, edit
game configuration, run PowerShell, or self-update. Those behaviors are normal
for this launcher, but they also overlap with common malware heuristics.

Before sharing public builds:

- build on GitHub Actions or another clean, reproducible machine;
- sign `RustyMangosLauncher.exe`, `authserver.exe`, `worldserver.exe`, and
  `RustyMangosSetup.exe` with an Authenticode code-signing certificate;
- publish the generated `SHA256SUMS.txt` next to the installer;
- submit the signed installer to Microsoft Security Intelligence if Defender
  still reports a false positive;
- avoid repacking the installer with third-party compressors or wrapper tools.

The packaging script signs automatically when either `RUSTY_MANGOS_SIGN_PFX`
or `RUSTY_MANGOS_SIGN_CERT_SHA1` is set:

```powershell
$env:RUSTY_MANGOS_SIGN_PFX = "C:\certs\rusty-mangos-signing.pfx"
$env:RUSTY_MANGOS_SIGN_PFX_PASSWORD = "<password>"
.\scripts\package-rusty-mangos-launcher.ps1
```

For a certificate installed in the Windows certificate store:

```powershell
$env:RUSTY_MANGOS_SIGN_CERT_SHA1 = "<certificate-thumbprint>"
.\scripts\package-rusty-mangos-launcher.ps1
```

## Useful Options

```powershell
.\scripts\rusty-mangos-launcher.cmd Install -ClientDir "C:\Games\World of Warcraft"
.\scripts\rusty-mangos-launcher.cmd Install -SkipWorldImport
.\scripts\rusty-mangos-launcher.cmd Install -ForceWorldImport
.\scripts\rusty-mangos-launcher.cmd Install -MMapMaps "0 1"
.\scripts\rusty-mangos-launcher.cmd Configure
```

`-MMapMaps` controls which map ids the launcher generates movement meshes for
on first run. The default `0 1` covers the two Vanilla outdoor continents and is
much faster than a full-world mmap build.

Default ports:

- MariaDB: `127.0.0.1:3307`
- Authserver: `127.0.0.1:13724`
- Worldserver: `127.0.0.1:18085`
- Dashboard: `http://127.0.0.1:9091/dashboard`
