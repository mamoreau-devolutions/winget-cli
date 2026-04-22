using Xunit;
using WinGetCore;

namespace WinGetCore.Tests;

public class VersionCompareTests
{
    [Theory]
    [InlineData("1.0.0", "1.0.0", 0)]
    [InlineData("1.0.1", "1.0.0", 1)]
    [InlineData("1.0.0", "1.0.1", -1)]
    [InlineData("2.0.0", "1.99.99", 1)]
    [InlineData("1.0", "1.0.0", 0)]
    [InlineData("0.98.1", "0.98.0", 1)]
    [InlineData("0.98.1", "0.98.1", 0)]
    [InlineData("10.0.0", "9.0.0", 1)]
    [InlineData("1.3.18-stable", "1.3.17-stable", 1)]
    public void CompareVersionStrings_ReturnsCorrectOrdering(string a, string b, int expected)
    {
        var result = RestSource.CompareVersionStrings(a, b);
        Assert.Equal(expected, Math.Sign(result));
    }
}

public class SourceStoreTests
{
    [Fact]
    public void DefaultSources_ContainsWingetAndMsstore()
    {
        var store = SourceStoreManager.Load();
        Assert.Contains(store.Sources, s => s.Name == "winget");
        Assert.Contains(store.Sources, s => s.Name == "msstore");
    }

    [Fact]
    public void RepositoryOpen_UsesCustomAppRoot()
    {
        var appRoot = TestPaths.CreateTempAppRoot();
        try
        {
            using var repo = Repository.Open(new RepositoryOptions
            {
                AppRoot = appRoot,
                UserAgent = "winget-dotnet-tests/1.0",
            });

            repo.AddSource("test", "https://example.com/test", SourceKind.Rest);

            Assert.Equal(Path.GetFullPath(appRoot), repo.AppRoot);
            Assert.True(File.Exists(Path.Combine(appRoot, "sources.json")));

            var store = SourceStoreManager.Load(appRoot);
            Assert.Contains(store.Sources, s => s.Name == "test");
        }
        finally
        {
            TestPaths.DeleteAppRoot(appRoot);
        }
    }
}

public class ModelsTests
{
    [Fact]
    public void SearchMatch_RequiredProperties()
    {
        var match = new SearchMatch
        {
            SourceName = "winget",
            SourceKind = SourceKind.PreIndexed,
            Id = "Test.Package",
            Name = "Test Package",
        };
        Assert.Equal("Test.Package", match.Id);
        Assert.Equal("Test Package", match.Name);
        Assert.Equal("winget", match.SourceName);
        Assert.Null(match.MatchCriteria);
    }

    [Fact]
    public void Installer_DefaultValues()
    {
        var installer = new Installer();
        Assert.Null(installer.Architecture);
        Assert.Null(installer.InstallerType);
        Assert.Null(installer.Url);
        Assert.Null(installer.Scope);
        Assert.Null(installer.ProductCode);
        Assert.True(installer.Switches.IsEmpty());
        Assert.Empty(installer.Commands);
        Assert.Empty(installer.PackageDependencies);
    }

    [Fact]
    public void Manifest_DefaultCollections()
    {
        var manifest = new Manifest
        {
            Id = "Test.Id",
            Name = "Test",
            Version = "1.0.0",
            Channel = "",
        };
        Assert.Empty(manifest.Tags);
        Assert.Empty(manifest.Installers);
        Assert.Empty(manifest.PackageDependencies);
        Assert.Empty(manifest.Documentation);
    }

    [Fact]
    public void ShowResult_ToStructuredDocument_UsesManifestSchema()
    {
        var result = new ShowResult
        {
            Package = new SearchMatch
            {
                SourceName = "winget",
                SourceKind = SourceKind.PreIndexed,
                Id = "Test.Package",
                Name = "Test Package",
                MatchCriteria = "Id",
            },
            Manifest = new Manifest
            {
                Id = "Test.Package",
                Name = "Test Package",
                Version = "1.2.3",
                Channel = "stable",
                Publisher = "Contoso",
                Description = "Structured output",
                Tags = ["utility"],
                PackageDependencies = ["Microsoft.VCRedist.2015+.x64"],
                Documentation =
                [
                    new Documentation { Label = "Docs", Url = "https://example.test/docs" }
                ],
                Installers =
                [
                    new Installer
                    {
                        Architecture = "x64",
                        InstallerType = "msix",
                        Url = "https://example.test/Test.Package.msix",
                        Sha256 = "ABC123",
                        Locale = "en-US",
                        Scope = "machine",
                        Switches = new InstallerSwitches { Silent = "/quiet" },
                        Commands = ["testpkg"],
                        PackageDependencies = ["Microsoft.UI.Xaml.2.8"],
                    }
                ],
            },
            SelectedInstaller = new Installer
            {
                Architecture = "x64",
                InstallerType = "msix",
                Url = "https://example.test/Test.Package.msix",
                Sha256 = "ABC123",
                Locale = "en-US",
                Scope = "machine",
                Switches = new InstallerSwitches { Silent = "/quiet" },
                Commands = ["testpkg"],
                PackageDependencies = ["Microsoft.UI.Xaml.2.8"],
            },
            CachedFiles = [@"C:\temp\cache\Test.Package.yaml"],
            Warnings = ["cache warmed"],
        };

        var document = result.ToStructuredDocument();
        var package = Assert.IsType<Dictionary<string, object?>>(document["Package"]);
        var manifest = Assert.IsType<Dictionary<string, object?>>(document["Manifest"]);
        var selectedInstaller = Assert.IsType<Dictionary<string, object?>>(document["SelectedInstaller"]);
        var cachedFiles = Assert.IsType<List<string>>(document["CachedFiles"]);
        var warnings = Assert.IsType<List<string>>(document["Warnings"]);

        Assert.Equal("Test.Package", package["PackageIdentifier"]);
        Assert.Equal("1.2.3", manifest["PackageVersion"]);

        var dependencies = Assert.IsType<Dictionary<string, object?>>(manifest["Dependencies"]);
        var packageDependencies = Assert.IsType<List<Dictionary<string, object?>>>(dependencies["PackageDependencies"]);
        Assert.Equal("Microsoft.VCRedist.2015+.x64", packageDependencies[0]["PackageIdentifier"]);

        var commands = Assert.IsType<List<string>>(selectedInstaller["Commands"]);
        Assert.Equal("testpkg", commands[0]);
        var switches = Assert.IsType<Dictionary<string, object?>>(selectedInstaller["InstallerSwitches"]);
        Assert.Equal("/quiet", switches["Silent"]);
        Assert.Equal(@"C:\temp\cache\Test.Package.yaml", cachedFiles[0]);
        Assert.Equal("cache warmed", warnings[0]);
    }

    [Fact]
    public void ParseYamlManifest_ReadsInstallerSwitches()
    {
        var yaml = """
            PackageIdentifier: Test.Package
            PackageVersion: 1.2.3
            PackageName: Test Package
            InstallerSwitches:
              SilentWithProgress: /SILENT
            Installers:
              - Architecture: x64
                InstallerType: inno
                InstallerUrl: https://example.test/Test.Package.exe
                InstallerSha256: ABC123
                InstallerSwitches:
                  Silent: /VERYSILENT
                  Interactive: /HELP
            """;

        var manifest = Repository.ParseYamlManifest(System.Text.Encoding.UTF8.GetBytes(yaml));
        var installer = Assert.Single(manifest.Installers);

        Assert.Equal("/SILENT", installer.Switches.SilentWithProgress);
        Assert.Equal("/VERYSILENT", installer.Switches.Silent);
        Assert.Equal("/HELP", installer.Switches.Interactive);
    }
}

public class PinStoreTests
{
    [Fact]
    public void AddListRemove_RoundTrips()
    {
        var appRoot = TestPaths.CreateTempAppRoot();
        try
        {
            PinStore.Add("Test.Package.Unit", "1.0.0", "winget", PinType.Pinning, appRoot);

            var pins = PinStore.List(appRoot);
            Assert.Contains(pins, p => p.PackageId == "Test.Package.Unit");
            var pin = pins.First(p => p.PackageId == "Test.Package.Unit");
            Assert.Equal("1.0.0", pin.Version);
            Assert.Equal(PinType.Pinning, pin.PinType);

            PinStore.Remove("Test.Package.Unit", appRoot);
            Assert.DoesNotContain(PinStore.List(appRoot), p => p.PackageId == "Test.Package.Unit");
        }
        finally
        {
            PinStore.Reset(appRoot);
            TestPaths.DeleteAppRoot(appRoot);
        }
    }
}

public class RepositoryParityTests
{
    [Fact]
    public void BuildArguments_UsesManifestSwitchesByMode()
    {
        var installer = new Installer
        {
            InstallerType = "inno",
            Switches = new InstallerSwitches
            {
                Silent = "/mysilent",
                SilentWithProgress = "/mysilentwithprogress",
                Interactive = "/myinteractive"
            }
        };

        Assert.Equal(["/mysilent"], InstallerDispatch.BuildArguments("inno", InstallerMode.Silent, installer));
        Assert.Equal(["/mysilentwithprogress"], InstallerDispatch.BuildArguments("inno", InstallerMode.SilentWithProgress, installer));
        Assert.Equal(["/myinteractive"], InstallerDispatch.BuildArguments("inno", InstallerMode.Interactive, installer));
    }

    [Fact]
    public void BuildArguments_UsesInnoDefaultsWhenManifestOmitsSwitches()
    {
        var installer = new Installer { InstallerType = "inno" };

        Assert.Equal(["/SP-", "/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART"], InstallerDispatch.BuildArguments("inno", InstallerMode.SilentWithProgress, installer));
        Assert.Equal(["/SP-", "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"], InstallerDispatch.BuildArguments("inno", InstallerMode.Silent, installer));
    }

    [Fact]
    public void BuildArguments_AppendsManifestAndCliSwitches()
    {
        var installer = new Installer
        {
            InstallerType = "msi",
            Switches = new InstallerSwitches
            {
                Custom = "ADDLOCAL=Core",
                Log = "/log \"<LOGPATH>\"",
                InstallLocation = "TARGETDIR=\"<INSTALLPATH>\"",
            }
        };

        var args = InstallerDispatch.BuildArguments(
            "msi",
            new InstallRequest
            {
                Query = new PackageQuery(),
                Mode = InstallerMode.Silent,
                LogPath = @"C:\temp\winget.log",
                Custom = "REBOOT=ReallySuppress",
                InstallLocation = @"C:\Apps\ShareX",
            },
            new Manifest { Id = "ShareX.ShareX", Name = "ShareX", Version = "19.0.2" },
            installer,
            @"C:\temp\ShareX.msi");

        Assert.Equal(["/i", @"C:\temp\ShareX.msi", "/quiet", "/norestart", "/log", @"C:\temp\winget.log", "ADDLOCAL=Core", "REBOOT=ReallySuppress", @"TARGETDIR=C:\Apps\ShareX"], args);
    }

    [Fact]
    public void BuildArguments_UsesOverrideInsteadOfSynthesizedArguments()
    {
        var installer = new Installer { InstallerType = "inno" };
        var args = InstallerDispatch.BuildArguments(
            "inno",
            new InstallRequest
            {
                Query = new PackageQuery(),
                Override = "/custom /args",
            },
            new Manifest { Id = "Test.Package", Name = "Test", Version = "1.0.0" },
            installer);

        Assert.Equal(["/custom", "/args"], args);
    }

    [Fact]
    public void GetArpSubkeyName_ExtractsRegistrySubkey()
    {
        Assert.Equal("ShareX", InstallerDispatch.GetArpSubkeyName(@"ARP\Machine\X64\ShareX"));
        Assert.Null(InstallerDispatch.GetArpSubkeyName(@"MSIX\ShareX_19.0.2_x64__name"));
    }

    [Fact]
    public void RegistryEntryMatchesInstalledPackage_UsesLocalIdentityInsteadOfCorrelatedId()
    {
        var installed = new ListMatch
        {
            Name = "ShareX",
            Id = "ShareX.ShareX",
            LocalId = @"ARP\Machine\X64\ShareX",
            InstalledVersion = "19.0.2",
            ProductCodes = [],
        };

        Assert.True(InstallerDispatch.RegistryEntryMatchesInstalledPackage("ShareX", "ShareX", null, installed));
        Assert.False(InstallerDispatch.RegistryEntryMatchesInstalledPackage("ShareX.ShareX", "ShareX.ShareX", null, installed));
    }

    [Fact]
    public void BuildUninstallCommand_AppendsSilentSwitchOnlyWhenNeeded()
    {
        Assert.Equal("\"C:\\Program Files\\ShareX\\unins000.exe\" /S",
            InstallerDispatch.BuildUninstallCommand("\"C:\\Program Files\\ShareX\\unins000.exe\"", silent: true, hasQuietUninstallCommand: false));
        Assert.Equal("\"C:\\Program Files\\ShareX\\unins000.exe\" /VERYSILENT",
            InstallerDispatch.BuildUninstallCommand("\"C:\\Program Files\\ShareX\\unins000.exe\" /VERYSILENT", silent: true, hasQuietUninstallCommand: false));
        Assert.Equal("\"C:\\Program Files\\ShareX\\unins000.exe\"",
            InstallerDispatch.BuildUninstallCommand("\"C:\\Program Files\\ShareX\\unins000.exe\"", silent: false, hasQuietUninstallCommand: false));
    }

    [Fact]
    public void SelectInstaller_PrefersRustStyleRanking()
    {
        var installers = new List<Installer>
        {
            new()
            {
                Architecture = "x64",
                InstallerType = "exe",
                Scope = "user",
                Locale = "en-US",
                Switches = new InstallerSwitches(),
            },
            new()
            {
                Architecture = "x64",
                InstallerType = "exe",
                Scope = "machine",
                Locale = "en-US",
                Switches = new InstallerSwitches(),
                Commands = ["powertoys"],
            },
        };

        var selected = Repository.SelectInstaller(installers, new PackageQuery { InstallerType = "exe" });

        Assert.NotNull(selected);
        Assert.Equal("machine", selected!.Scope);
    }

    [Fact]
    public void SelectInstaller_PrefersLanguageFallbackOverMismatchedLocale()
    {
        var installers = new List<Installer>
        {
            new() { Architecture = "x64", InstallerType = "exe", Locale = "fr-FR", Switches = new InstallerSwitches() },
            new() { Architecture = "x64", InstallerType = "exe", Locale = "en-GB", Switches = new InstallerSwitches() },
        };

        var selected = Repository.SelectInstaller(installers, new PackageQuery
        {
            InstallerType = "exe",
            Locale = "en-US",
        });

        Assert.NotNull(selected);
        Assert.Equal("en-GB", selected!.Locale);
    }
}

file static class TestPaths
{
    public static string CreateTempAppRoot() =>
        Path.Combine(Path.GetTempPath(), "winget-dotnet-tests", Guid.NewGuid().ToString("N"));

    public static void DeleteAppRoot(string appRoot)
    {
        if (Directory.Exists(appRoot))
            Directory.Delete(appRoot, recursive: true);
    }
}
