; rbara.iss — Inno Setup script for rbara (Windows x64)
;
; Build with: iscc /DAppVersion=x.y.z installer\windows\rbara.iss
; Or via the orchestrator: installer\windows\build-installer.ps1
;
; Inputs (passed via /D on the command line):
;   AppVersion       — semver string pulled from rbara/Cargo.toml
;   PdfiumChromium   — chromium build number used for the bundled pdfium.dll
;
; Layout consumed:
;   ..\..\target\release\rbara.exe
;   .\assets\pdfium.dll
;
; Output:
;   .\dist\rbara-setup-{AppVersion}-x64.exe

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef FileVersion
  #define FileVersion AppVersion
#endif
#ifndef PdfiumChromium
  #define PdfiumChromium "unknown"
#endif

#define AppName        "rbara"
#define AppPublisher   "rustybara"
#define AppURL         "https://github.com/"
#define AppExeName     "rbara.exe"

[Setup]
AppId={{A6E1B3D2-9F4E-4F2B-9D8A-RBARA00000001}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
VersionInfoVersion={#FileVersion}
VersionInfoDescription=rbara prepress CLI/TUI (bundles pdfium chromium/{#PdfiumChromium})

DefaultDirName={autopf}\rbara
DefaultGroupName=rbara
DisableProgramGroupPage=yes
DisableDirPage=auto
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

OutputDir=dist
OutputBaseFilename=rbara-setup-{#AppVersion}-x64
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\{#AppExeName}
UninstallDisplayName={#AppName} {#AppVersion}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "addtopath"; Description: "Add rbara to my user PATH (recommended)"; GroupDescription: "Shell integration:"

[Files]
Source: "..\..\target\release\rbara.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\pdfium.dll";              DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-GPL-3.0";          DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"

[Run]
Filename: "{app}\{#AppExeName}"; Parameters: "--help"; \
  Description: "Verify install (rbara --help)"; \
  Flags: postinstall nowait skipifsilent runhidden

; --- PATH management (per-user, no admin required) -----------------------------
; We append {app} to HKCU\Environment\Path on install (when the user opted in)
; and remove it on uninstall. Inno Setup's [Registry] entries with
; ValueType: expandsz and Check: NeedsAddPath() avoid duplicate entries.

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; \
  ValueData: "{olddata};{app}"; \
  Check: NeedsAddPath(ExpandConstant('{app}')); \
  Tasks: addtopath; \
  Flags: preservestringtype

[Code]
function NeedsAddPath(Param: string): Boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
  begin
    Result := True;
    exit;
  end;
  // Look for ;param; in ;OrigPath;  (case-insensitive)
  Result := Pos(';' + Lowercase(Param) + ';', ';' + Lowercase(OrigPath) + ';') = 0;
end;

procedure RemoveFromPath(PathToRemove: string);
var
  OrigPath, NewPath: string;
  P: Integer;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath) then
    exit;
  NewPath := ';' + OrigPath + ';';
  P := Pos(';' + Lowercase(PathToRemove) + ';', Lowercase(NewPath));
  if P = 0 then
    exit;
  Delete(NewPath, P, Length(PathToRemove) + 1);
  // Trim leading/trailing semicolons
  if (Length(NewPath) > 0) and (NewPath[1] = ';') then Delete(NewPath, 1, 1);
  if (Length(NewPath) > 0) and (NewPath[Length(NewPath)] = ';') then
    Delete(NewPath, Length(NewPath), 1);
  RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', NewPath);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RemoveFromPath(ExpandConstant('{app}'));
end;
