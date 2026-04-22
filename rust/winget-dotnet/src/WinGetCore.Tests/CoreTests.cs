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
}

public class PinStoreTests
{
    [Fact]
    public void AddListRemove_RoundTrips()
    {
        PinStore.Add("Test.Package.Unit", "1.0.0", "winget", PinType.Pinning);
        try
        {
            var pins = PinStore.List();
            Assert.Contains(pins, p => p.PackageId == "Test.Package.Unit");
            var pin = pins.First(p => p.PackageId == "Test.Package.Unit");
            Assert.Equal("1.0.0", pin.Version);
            Assert.Equal(PinType.Pinning, pin.PinType);
        }
        finally
        {
            PinStore.Remove("Test.Package.Unit");
        }
        Assert.DoesNotContain(PinStore.List(), p => p.PackageId == "Test.Package.Unit");
    }
}

public class RepositoryParityTests
{
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
            },
            new()
            {
                Architecture = "x64",
                InstallerType = "exe",
                Scope = "machine",
                Locale = "en-US",
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
            new() { Architecture = "x64", InstallerType = "exe", Locale = "fr-FR" },
            new() { Architecture = "x64", InstallerType = "exe", Locale = "en-GB" },
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
