; Lunar Browser installer for Windows (Inno Setup 6).
; Built by scripts/package-windows.sh, which passes SourceDir, AppVersion,
; Arch, OutputDir and OutputName.

[Setup]
AppId={{6C7D3E1A-4F0B-4C55-9E2B-1A5E3B7C9D20}
AppName=Lunar Browser
AppVersion={#AppVersion}
AppPublisher=Lunar
AppComments=Independent web browser with its own engine and an ASCII look
DefaultDirName={autopf}\Lunar Browser
DefaultGroupName=Lunar Browser
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\lunar-browser.exe
SetupIconFile={#SourceDir}\lunar-browser.ico
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
Name: "{autoprograms}\Lunar Browser"; Filename: "{app}\lunar-browser.exe"
Name: "{autodesktop}\Lunar Browser"; Filename: "{app}\lunar-browser.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\lunar-browser.exe"; Description: "Start Lunar Browser"; Flags: nowait postinstall skipifsilent
