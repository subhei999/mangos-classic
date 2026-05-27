#define MyAppName "Rusty MaNGOS"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "Rusty MaNGOS"
#define MyAppExeName "RustyMangosLauncher.exe"

[Setup]
AppId={{E174BC31-598D-4CB7-9C91-2F17161CE255}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL=https://github.com/subhei999/rusty-mangos
AppSupportURL=https://github.com/subhei999/rusty-mangos/issues
AppUpdatesURL=https://github.com/subhei999/rusty-mangos/releases
DefaultDirName={autopf}\Rusty MaNGOS
DefaultGroupName=Rusty MaNGOS
DisableProgramGroupPage=yes
OutputDir=..\target\launcher-package\installer
OutputBaseFilename=RustyMangosSetup
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
SetupLogging=yes
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription=Rusty MaNGOS Launcher Setup
VersionInfoProductName={#MyAppName}
VersionInfoProductVersion={#MyAppVersion}
VersionInfoVersion={#MyAppVersion}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\launcher-package\app\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Rusty MaNGOS Launcher"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\Start Rusty MaNGOS"; Filename: "{app}\scripts\rusty-mangos-launcher.cmd"; Parameters: "Start"
Name: "{group}\Stop Rusty MaNGOS"; Filename: "{app}\scripts\rusty-mangos-launcher.cmd"; Parameters: "Stop"
Name: "{group}\Rusty MaNGOS Status"; Filename: "{app}\scripts\rusty-mangos-launcher.cmd"; Parameters: "Status"
Name: "{autodesktop}\Rusty MaNGOS Launcher"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch Rusty MaNGOS"; Flags: nowait postinstall skipifsilent
