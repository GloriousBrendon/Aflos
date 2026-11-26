; Aflos's Windows installer (Inno Setup 6). Build with installer\build.ps1.
; Per-user: no admin prompt, installs to %LOCALAPPDATA%\Programs\Aflos, and
; leaves your accounts and chats (%APPDATA%\aflos, and %APPDATA%\mesh from
; before the rename) alone on uninstall. Same AppId as Mesh's setup, so it
; updates a Mesh install in place (in its old Programs\Mesh folder).

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
AppPublisherURL=https://github.com/GloriousBrendon/Aflos
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
; The welcome page says who Aflos is for (TERMS.md); silent updates skip it.
DisableWelcomePage=no

[Messages]
WelcomeLabel2=This will install [name/ver] on your computer.%n%nAflos is for people 18 and over. By installing it, you confirm you are at least 18.

[Tasks]
Name: desktopicon; Description: "Create a desktop shortcut"; Flags: unchecked
Name: autostart; Description: "Start Aflos when I sign in to Windows"; Flags: unchecked

[InstallDelete]
; What Mesh (before the rename to Aflos) installed under its old names.
Type: files; Name: "{app}\Mesh.exe"
Type: files; Name: "{app}\mesh-wa.exe"
Type: files; Name: "{app}\mesh-dc.exe"
Type: files; Name: "{app}\mesh.png"
Type: files; Name: "{userprograms}\Mesh.lnk"
Type: files; Name: "{userdesktop}\Mesh.lnk"

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
; Mesh's autostart and notification entries, from before the rename.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Mesh"; Flags: deletevalue
Root: HKCU; Subkey: "Software\Classes\AppUserModelId\Mesh.Mesh"; ValueType: none; Flags: deletekey

[Run]
Filename: "{app}\Aflos.exe"; Description: "Start Aflos"; Flags: nowait postinstall skipifsilent
; An update from inside Aflos runs this silently: start the new Aflos when done.
Filename: "{app}\Aflos.exe"; Flags: nowait; Check: WizardSilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM Aflos.exe /IM aflos-wa.exe /IM aflos-dc.exe /IM mesh-node.exe"; Flags: runhidden; RunOnceId: "StopAflos"

[Code]
// Restart Manager only closes programs whose files this setup replaces, and
// Mesh.exe (before the rename) isn't one: stop it so its old files can go.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM Mesh.exe /IM mesh-wa.exe /IM mesh-dc.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Result := '';
end;
