#ifndef AppVersion
  #define AppVersion "0.8.2"
#endif

#ifdef TestAppId
  #define ShortcutName "fpdf Installer Smoke"
#else
  #define ShortcutName "fpdf"
#endif

[Setup]
#ifdef TestAppId
AppId={#TestAppId}
#else
AppId={{84BDF48E-8624-49AA-B59C-C0501CA586CF}
#endif
AppName={#ShortcutName}
AppVersion={#AppVersion}
AppPublisher=libo-libo
AppPublisherURL=https://github.com/libo-libo/FPDF
AppSupportURL=https://github.com/libo-libo/FPDF/issues
#ifdef TestAppId
DefaultDirName={localappdata}\Programs\fpdf-installer-smoke
#else
DefaultDirName={localappdata}\Programs\fpdf
#endif
DefaultGroupName={#ShortcutName}
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
UninstallDisplayIcon={app}\fpdf.exe,0
SetupIconFile=..\assets\fpdf.ico
CloseApplications=yes
CloseApplicationsFilter=fpdf.exe,node.exe
RestartApplications=no
ChangesAssociations=no
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
OutputDir=..\dist
#ifdef TestAppId
OutputBaseFilename=fpdf-{#AppVersion}-windows-x64-setup-smoke
#else
OutputBaseFilename=fpdf-{#AppVersion}-windows-x64-setup
#endif
LicenseFile=..\LICENSE
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Files]
Source: "..\dist\fpdf\*"; DestDir: "{app}"; Excludes: "agent.txt,githubtoken.txt,.env"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#ShortcutName}"; Filename: "{app}\fpdf.exe"; IconFilename: "{app}\fpdf.exe"; IconIndex: 0; AppUserModelID: "libolibo.fpdf"
Name: "{group}\User Guide"; Filename: "{app}\fpdf-user-guide-zh-CN.pdf"
Name: "{autodesktop}\{#ShortcutName}"; Filename: "{app}\fpdf.exe"; IconFilename: "{app}\fpdf.exe"; IconIndex: 0; AppUserModelID: "libolibo.fpdf"; Tasks: desktopicon

Name: "{group}\卸载 fpdf（清除全部配置）"; Filename: "{uninstallexe}"; IconFilename: "{app}\fpdf.exe"; IconIndex: 0

[UninstallDelete]
Type: files; Name: "{app}\agent.txt"
Type: files; Name: "{app}\reading-theme.txt"
Type: files; Name: "{app}\fpdf*.log"
Type: dirifempty; Name: "{app}"

[Run]
Filename: "{app}\fpdf.exe"; Description: "Launch fpdf"; Flags: nowait postinstall skipifsilent

[Code]
function GetFileAttributesW(const FileName: String): Cardinal;
  external 'GetFileAttributesW@kernel32.dll stdcall';

// Close only processes loaded from this exact installation. Other portable
// readers and unrelated Node applications are never selected by process name.
procedure CloseInstalledProcesses;
var
  Locator, Services, Processes, Process: Variant;
  Index: Integer;
  Closed: Boolean;
  Executable, AppExecutable, AiExecutable: String;
begin
  Closed := False;
  AppExecutable := ExpandFileName(ExpandConstant('{app}\fpdf.exe'));
  AiExecutable := ExpandFileName(ExpandConstant('{app}\ai\node.exe'));
  try
    Locator := CreateOleObject('WbemScripting.SWbemLocator');
    Services := Locator.ConnectServer('', 'root\CIMV2');
    Processes := Services.ExecQuery('SELECT ExecutablePath FROM Win32_Process WHERE Name = ''fpdf.exe'' OR Name = ''node.exe''');
    for Index := 0 to Processes.Count - 1 do begin
      Process := Processes.ItemIndex(Index);
      if not VarIsNull(Process.ExecutablePath) then begin
        Executable := ExpandFileName(Process.ExecutablePath);
        if (CompareText(Executable, AppExecutable) = 0) or
           (CompareText(Executable, AiExecutable) = 0) then begin
          Log('Closing an fpdf process from this installation.');
          Process.Terminate(0);
          Closed := True;
        end;
      end;
    end;
  except
    Log('Could not close a running fpdf process automatically.');
  end;
  if Closed then Sleep(300);
end;

// Preserve user PDFs and never recurse into junctions or symbolic links.
procedure RemoveOwnedTree(const Directory: String);
var
  Found: TFindRec;
  Attributes: Cardinal;
  Name: String;
begin
  Attributes := GetFileAttributesW(Directory);
  if Attributes = $FFFFFFFF then Exit;
  if (Attributes and $400) <> 0 then begin
    RemoveDir(Directory);
    Exit;
  end;
  if FindFirst(AddBackslash(Directory) + '*', Found) then begin
    try
      repeat
        if (Found.Name <> '.') and (Found.Name <> '..') then begin
          Name := AddBackslash(Directory) + Found.Name;
          if (Found.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0 then begin
            if (Found.Attributes and $400) = 0 then
              RemoveOwnedTree(Name)
            else
              RemoveDir(Name);
          end else if CompareText(ExtractFileExt(Found.Name), '.pdf') <> 0 then
            DeleteFile(Name);
        end;
      until not FindNext(Found);
    finally
      FindClose(Found);
    end;
  end;
  RemoveDir(Directory);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    CloseInstalledProcesses;
  if CurUninstallStep = usPostUninstall then begin
#ifdef TestAppId
    // Smoke builds never touch the real user's app settings.
    RemoveOwnedTree(ExpandConstant('{app}\smoke-user-data\roaming\fpdf'));
    RemoveOwnedTree(ExpandConstant('{app}\smoke-user-data\local\fpdf'));
#else
    RemoveOwnedTree(ExpandConstant('{userappdata}\fpdf'));
    RemoveOwnedTree(ExpandConstant('{localappdata}\fpdf'));
#endif
    RemoveOwnedTree(ExpandConstant('{app}\logs'));
    RemoveOwnedTree(ExpandConstant('{app}\cache'));
    RemoveOwnedTree(ExpandConstant('{app}\ai'));
    RemoveDir(ExpandConstant('{app}'));
  end;
end;
