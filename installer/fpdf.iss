#ifndef AppVersion
  #define AppVersion "0.7.0"
#endif

[Setup]
AppId={{84BDF48E-8624-49AA-B59C-C0501CA586CF}
AppName=fpdf
AppVersion={#AppVersion}
AppPublisher=libo-libo
AppPublisherURL=https://github.com/libo-libo/FPDF
AppSupportURL=https://github.com/libo-libo/FPDF/issues
DefaultDirName={localappdata}\Programs\fpdf
DefaultGroupName=fpdf
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
UninstallDisplayIcon={app}\fpdf.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
OutputDir=..\dist
OutputBaseFilename=fpdf-{#AppVersion}-windows-x64-setup
LicenseFile=..\LICENSE
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Files]
Source: "..\dist\fpdf\*"; DestDir: "{app}"; Excludes: "agent.txt,githubtoken.txt,.env"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\fpdf"; Filename: "{app}\fpdf.exe"
Name: "{group}\User Guide"; Filename: "{app}\fpdf-user-guide-zh-CN.pdf"
Name: "{autodesktop}\fpdf"; Filename: "{app}\fpdf.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\fpdf.exe"; Description: "Launch fpdf"; Flags: nowait postinstall skipifsilent
