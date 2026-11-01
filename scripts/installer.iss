; Installer for Windows (Inno Setup 6), used for both browsers.
; Built by scripts/package-windows.sh, which passes AppName, AppExe, AppGuid,
; AppComments, SourceDir, AppVersion, Arch, OutputDir and OutputName.

[Setup]
AppId={{{#AppGuid}}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Lunar
AppComments={#AppComments}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\{#AppExe}
SetupIconFile={#SourceDir}\app.ico
ArchitecturesAllowed={#Arch}
ArchitecturesInstallIn64BitMode={#Arch}
OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequiredOverridesAllowed=dialog

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "Start {#AppName}"; Flags: nowait postinstall skipifsilent
