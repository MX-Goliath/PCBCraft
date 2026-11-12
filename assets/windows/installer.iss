#ifndef AppVersion
  #error AppVersion is required
#endif
#ifndef SourceDir
  #error SourceDir is required
#endif
#ifndef OutputDir
  #error OutputDir is required
#endif

[Setup]
AppId={{CFADACB0-6CB6-4BA7-A101-FB78F304D8DA}
AppName=PCBCraft
AppVersion={#AppVersion}
AppPublisher=MX-Goliath
AppPublisherURL=https://github.com/MX-Goliath/PCBCraft
DefaultDirName={localappdata}\Programs\PCBCraft
DefaultGroupName=PCBCraft
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename=PCBCraft-{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\pcbcraft.exe
CloseApplications=yes
RestartApplications=no

[Files]
Source: "{#SourceDir}\pcbcraft.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\*.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#SourceDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#SourceDir}\build-info.json"; DestDir: "{app}"; Flags: ignoreversion

[InstallDelete]
; These are application-owned files. Remove obsolete runtime DLLs/notices too.
Type: files; Name: "{app}\*.dll"
Type: filesandordirs; Name: "{app}\licenses"

[Icons]
Name: "{group}\PCBCraft"; Filename: "{app}\pcbcraft.exe"
Name: "{autodesktop}\PCBCraft"; Filename: "{app}\pcbcraft.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked
