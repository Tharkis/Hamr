; Windows installer for Hamr, built with Inno Setup 6:
;   iscc /DAppVersion=1.0.0 packaging\windows\hamr.iss
; Expects target\release\hamr.exe and, if found, the VC++ runtime DLLs in dist\windows.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{332C7E7D-3C22-45A6-961E-87B6A089D793}
AppName=Hamr
AppVersion={#AppVersion}
AppVerName=Hamr {#AppVersion}
AppPublisher=Hamr contributors
AppPublisherURL=https://github.com/Tharkis/Hamr
AppSupportURL=https://github.com/Tharkis/Hamr/issues
DefaultDirName={autopf}\Hamr
DefaultGroupName=Hamr
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=Hamr-{#AppVersion}-windows-x64-setup
SetupIconFile=..\..\assets\icon\hamr.ico
UninstallDisplayIcon={app}\hamr.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\..\target\release\hamr.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\dist\windows\*.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Hamr"; Filename: "{app}\hamr.exe"
Name: "{autodesktop}\Hamr"; Filename: "{app}\hamr.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\hamr.exe"; Description: "{cm:LaunchProgram,Hamr}"; Flags: nowait postinstall skipifsilent
