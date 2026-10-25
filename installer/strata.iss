; The Windows installer (Inno Setup 6). Built by the release workflow after `cargo build --release`:
;   ISCC.exe /DAppVersion=0.6.1 installer\strata.iss
; It writes dist\strata-setup-x64.exe. Installs for the current user by default (no admin needed); the first page
; offers "all users" instead.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
; Never change the AppId: Windows uses it to find the installed copy for updates and uninstalling.
AppId={{881E3266-F99F-4C16-8D52-9017365CFE2D}
AppName=Strata
AppVersion={#AppVersion}
AppVerName=Strata {#AppVersion}
AppPublisher=Gowtham Jeyabalan
AppPublisherURL=https://github.com/gowthamjeyabalan/strata
AppSupportURL=https://github.com/gowthamjeyabalan/strata/issues
AppUpdatesURL=https://github.com/gowthamjeyabalan/strata/releases
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\Strata
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\dist
OutputBaseFilename=strata-setup-x64
SetupIconFile=..\brand\ico\strata.ico
UninstallDisplayIcon={app}\strata.exe
UninstallDisplayName=Strata
LicenseFile=..\LICENSE
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
ChangesAssociations=yes
; a running Strata is closed before its files are replaced
CloseApplications=yes

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "association"; Description: "Open .strata (and older .overlay) boards with Strata"; GroupDescription: "File types:"

[Files]
Source: "..\target\release\strata.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\examples\demo.strata"; DestDir: "{app}\examples"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Strata"; Filename: "{app}\strata.exe"
Name: "{autodesktop}\Strata"; Filename: "{app}\strata.exe"; Tasks: desktopicon

[Registry]
; .strata boards open in Strata (in HKCU for a per-user install, HKLM for all users)
Root: HKA; Subkey: "Software\Classes\.strata"; ValueType: string; ValueName: ""; ValueData: "Strata.Board"; Flags: uninsdeletevalue; Tasks: association
Root: HKA; Subkey: "Software\Classes\.strata\OpenWithProgids"; ValueType: string; ValueName: "Strata.Board"; ValueData: ""; Flags: uninsdeletevalue; Tasks: association
Root: HKA; Subkey: "Software\Classes\Strata.Board"; ValueType: string; ValueName: ""; ValueData: "Strata board"; Flags: uninsdeletekey; Tasks: association
Root: HKA; Subkey: "Software\Classes\Strata.Board\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\strata.exe,0"; Tasks: association
Root: HKA; Subkey: "Software\Classes\Strata.Board\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\strata.exe"" ""%1"""; Tasks: association
; boards saved while the app was called Overlay open in Strata too
Root: HKA; Subkey: "Software\Classes\.overlay"; ValueType: string; ValueName: ""; ValueData: "Strata.Board"; Flags: uninsdeletevalue; Tasks: association
Root: HKA; Subkey: "Software\Classes\.overlay\OpenWithProgids"; ValueType: string; ValueName: "Strata.Board"; ValueData: ""; Flags: uninsdeletevalue; Tasks: association
; "Open with" lists Strata for its boards
Root: HKA; Subkey: "Software\Classes\Applications\strata.exe"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\strata.exe\SupportedTypes"; ValueType: string; ValueName: ".strata"; ValueData: ""; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\strata.exe\SupportedTypes"; ValueType: string; ValueName: ".overlay"; ValueData: ""

[Run]
Filename: "{app}\strata.exe"; Description: "{cm:LaunchProgram,Strata}"; Flags: nowait postinstall skipifsilent
