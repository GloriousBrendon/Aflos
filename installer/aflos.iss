; Aflos's Windows installer (Inno Setup 6). Build with installer\build.ps1.
; Per-user: no admin prompt, installs to %LOCALAPPDATA%\Programs\Aflos, and
; leaves your accounts and chats (%APPDATA%\mesh) alone on uninstall.

#define Root ".."
; What build.ps1 passes: the Mesh-Node checkout, the Rust target the
; binaries were built for, and the two commits they were built from.
#ifndef Node
  #define Node Root + "\..\Mesh-Node"
#endif
#ifndef Target
  #define Target "x86_64-pc-windows-msvc"
#endif
#ifndef AppCommit
  #define AppCommit "unknown"
#endif
#ifndef NodeCommit
  #define NodeCommit "unknown"
#endif
#define Bin "\target\" + Target + "\release\"
#define AppExe Root + Bin + "aflos.exe"
#define AppVersion GetVersionNumbersString(AppExe)
#define Built AppVersion + " (app " + AppCommit + ", engine " + NodeCommit + ")"

[Setup]
AppId={{D5884734-20BD-4787-83B8-C0C7491957AD}
AppName=Aflos
AppVersion={#AppVersion}
AppVerName=Aflos {#AppVersion}
AppPublisher=Aflos
AppPublisherURL=https://github.com/GloriousBrendon/Mesh-Voice
VersionInfoDescription=Aflos Setup
VersionInfoVersion={#AppVersion}
VersionInfoProductTextVersion={#Built}
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\Aflos
DisableProgramGroupPage=yes
DisableDirPage=yes
SetupIconFile={#Root}\crates\aflos\assets\aflos.ico
UninstallDisplayIcon={app}\Aflos.exe
UninstallDisplayName=Aflos
OutputDir={#Root}\dist
OutputBaseFilename=Aflos-Setup-{#AppVersion}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Aflos and its helpers may be running: close them (Restart Manager) first.
CloseApplications=force
RestartApplications=no

[Tasks]
Name: desktopicon; Description: "Create a desktop shortcut"; Flags: unchecked
Name: autostart; Description: "Start Aflos when I sign in to Windows"; Flags: unchecked

[Files]
Source: "{#AppExe}"; DestDir: "{app}"; DestName: "Aflos.exe"; Flags: ignoreversion
Source: "{#Root}\aflos-wa{#Bin}aflos-wa.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\aflos-dc{#Bin}aflos-dc.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Node}{#Bin}mesh-node.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Root}\crates\aflos\assets\aflos.png"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Aflos"; Filename: "{app}\Aflos.exe"; Comment: "Matrix, Discord and WhatsApp in one place"
Name: "{userdesktop}\Aflos"; Filename: "{app}\Aflos.exe"; Tasks: desktopicon

[Registry]
; Aflos's name and icon on its notifications (see `notify` in main.rs), instead of PowerShell's.
Root: HKCU; Subkey: "Software\Classes\AppUserModelId\Aflos.Aflos"; ValueType: string; ValueName: "DisplayName"; ValueData: "Aflos"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\AppUserModelId\Aflos.Aflos"; ValueType: string; ValueName: "IconUri"; ValueData: "{app}\aflos.png"
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Aflos"; ValueData: """{app}\Aflos.exe"""; Tasks: autostart; Flags: uninsdeletevalue

[Run]
Filename: "{app}\Aflos.exe"; Description: "Start Aflos"; Flags: nowait postinstall skipifsilent
; An update from inside Aflos runs this silently: start the new Aflos when done.
Filename: "{app}\Aflos.exe"; Flags: nowait; Check: WizardSilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM Aflos.exe /IM aflos-wa.exe /IM aflos-dc.exe /IM mesh-node.exe"; Flags: runhidden; RunOnceId: "StopMesh"
