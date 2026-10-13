; Secretarybird installer (Inno Setup 6).
; Build:  cargo build --release
;         ISCC.exe /DAppVersion=0.1.0 installer\secbird.iss
; Output: target\installer\Secretarybird-Setup-<version>.exe
;
; Installs for the current user only (no administrator prompt) into
; %LOCALAPPDATA%\Programs\Secretarybird. Settings stay in %APPDATA%\Secretarybird.

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif
#define AppName "Secretarybird"
#define TrayExe "secbird-tray.exe"

[Setup]
; Never change AppId: Windows uses it to find the existing install on upgrade.
AppId={{6E3A1F52-8C4D-4B7E-9A21-5D0C3B8F7E14}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Secretarybird contributors
AppPublisherURL=https://github.com/infinityInfinite/secbird
AppSupportURL=https://github.com/infinityInfinite/secbird/issues
AppUpdatesURL=https://github.com/infinityInfinite/secbird/releases
PrivilegesRequired=lowest
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
DisableReadyPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
SourceDir=..
OutputDir=target\installer
OutputBaseFilename=Secretarybird-Setup-{#AppVersion}
SetupIconFile=assets\secretarybird.ico
UninstallDisplayIcon={app}\{#TrayExe}
UninstallDisplayName={#AppName}
LicenseFile=LICENSE-MIT
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
; The tray is closed by PrepareToInstall below; skip the Restart Manager dialog.
CloseApplications=no
ChangesEnvironment=yes

[Tasks]
Name: "autostart"; Description: "Start {#AppName} with Windows (applies your profiles at sign-in)"
Name: "desktopicon"; Description: "Desktop shortcut"; Flags: unchecked
Name: "addtopath"; Description: "Add the secbird command line to PATH"; Flags: unchecked

[Files]
Source: "target\release\secbird-tray.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\secbird.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion
Source: "NOTICE"; DestDir: "{app}"; DestName: "NOTICE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#TrayExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#TrayExe}"; Tasks: desktopicon

[Registry]
; Same value the tray's "Start with Windows" menu item reads and writes.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#AppName}"; ValueData: """{app}\{#TrayExe}"""; Tasks: autostart

[Run]
Filename: "{app}\{#TrayExe}"; Description: "Launch {#AppName}"; Flags: postinstall nowait skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/IM {#TrayExe} /F"; Flags: runhidden; RunOnceId: "CloseTray"

[Code]
const
  TrayClass = 'SecretarybirdTray';
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';
  EnvKey = 'Environment';

var
  TrayWasRunning: Boolean;

procedure CloseTray;
var
  Code: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM {#TrayExe} /F', '', SW_HIDE, ewWaitUntilTerminated, Code);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  // Upgrades: the running tray holds its exe open. Note it so a silent update restarts it.
  TrayWasRunning := FindWindowByClassName(TrayClass) <> 0;
  CloseTray;
  Result := '';
end;

{ User PATH: add or remove the install folder as one ';'-separated entry. }
function PathHas(const Path, Dir: String): Boolean;
begin
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Path) + ';') > 0;
end;

procedure AddToPath(const Dir: String);
var
  Path: String;
begin
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Path) then Path := '';
  if PathHas(Path, Dir) then Exit;
  if (Path <> '') and (Copy(Path, Length(Path), 1) <> ';') then Path := Path + ';';
  RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Path + Dir);
end;

procedure RemoveFromPath(const Dir: String);
var
  Path: String;
  P: Integer;
begin
  if not RegQueryStringValue(HKCU, EnvKey, 'Path', Path) then Exit;
  if not PathHas(Path, Dir) then Exit;
  Path := ';' + Path + ';';
  P := Pos(';' + Uppercase(Dir) + ';', Uppercase(Path));
  Delete(Path, P, Length(Dir) + 1);
  Path := Copy(Path, 2, Length(Path) - 2);
  RegWriteExpandStringValue(HKCU, EnvKey, 'Path', Path);
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Code: Integer;
begin
  if CurStep <> ssPostInstall then Exit;
  if WizardIsTaskSelected('addtopath') then
    AddToPath(ExpandConstant('{app}'));
  // The "Launch" checkbox covers normal installs; silent updates bring the tray back by themselves.
  if WizardSilent and TrayWasRunning then
    Exec(ExpandConstant('{app}\{#TrayExe}'), '', '', SW_SHOWNORMAL, ewNoWait, Code);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Settings, Msg: String;
begin
  if CurUninstallStep <> usPostUninstall then Exit;
  // The tray menu can turn autostart on after install, so always remove it.
  RegDeleteValue(HKCU, RunKey, '{#AppName}');
  RemoveFromPath(ExpandConstant('{app}'));

  Settings := ExpandConstant('{userappdata}\{#AppName}');
  if UninstallSilent or not DirExists(Settings) then Exit;
  Msg := 'Also delete your profiles and settings?' + #13#10 + Settings + #13#10#13#10 +
         'Choose No to keep them for a later reinstall.';
  if FileExists(Settings + '\debloat-backup.json') then
    Msg := Msg + #13#10#13#10 + 'It also holds the backup made by "secbird debloat apply". ' +
           'If you may want Razer''s services back later, choose No.';
  if MsgBox(Msg, mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
    DelTree(Settings, True, True, True);
end;
