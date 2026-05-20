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
- primary **Install / Start** or **Restart** action;
- quick **Start**, **Stop**, **Dashboard**, and app-data shortcuts;
- setup page for choosing the WoW client folder;
- logs page with backend output;
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

## Useful Options

```powershell
.\scripts\rusty-mangos-launcher.cmd Install -ClientDir "C:\Games\World of Warcraft"
.\scripts\rusty-mangos-launcher.cmd Install -SkipWorldImport
.\scripts\rusty-mangos-launcher.cmd Install -ForceWorldImport
.\scripts\rusty-mangos-launcher.cmd Configure
```

Default ports:

- MariaDB: `127.0.0.1:3307`
- Authserver: `127.0.0.1:13724`
- Worldserver: `127.0.0.1:18085`
- Dashboard: `http://127.0.0.1:9091/dashboard`
