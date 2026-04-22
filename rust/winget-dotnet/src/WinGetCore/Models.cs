using System.Text.Json.Serialization;

namespace WinGetCore;

[JsonConverter(typeof(JsonStringEnumConverter<SourceKind>))]
public enum SourceKind
{
    PreIndexed,
    Rest
}

public enum PinType
{
    Pinning,
    Blocking,
    Gating
}

public record SourceRecord
{
    public required string Name { get; init; }
    public required SourceKind Kind { get; init; }
    public required string Arg { get; init; }
    public required string Identifier { get; init; }
    public DateTime? LastUpdate { get; set; }
    public string? SourceVersion { get; set; }
}

/// <summary>
/// Library-hosting options for the WinGet core API.
/// </summary>
public record RepositoryOptions
{
    public string? AppRoot { get; init; }
    public string UserAgent { get; init; } = "winget-dotnet/0.1";
}

public record PackageQuery
{
    public string? Query { get; init; }
    public string? Id { get; init; }
    public string? Name { get; init; }
    public string? Moniker { get; init; }
    public string? Tag { get; init; }
    public string? Command { get; init; }
    public string? Source { get; init; }
    public int? Count { get; init; }
    public bool Exact { get; init; }
    public string? Version { get; init; }
    public string? Channel { get; init; }
    public string? Locale { get; init; }
    public string? InstallerType { get; init; }
    public string? InstallerArchitecture { get; init; }
    public string? InstallScope { get; init; }
}

public record ListQuery
{
    public string? Query { get; init; }
    public string? Id { get; init; }
    public string? Name { get; init; }
    public string? Moniker { get; init; }
    public string? Tag { get; init; }
    public string? Command { get; init; }
    public string? Source { get; init; }
    public int? Count { get; init; }
    public bool Exact { get; init; }
    public string? InstallScope { get; init; }
    public bool UpgradeOnly { get; init; }
    public bool IncludeUnknown { get; init; }
    public bool IncludePinned { get; init; }
}

public record SearchMatch
{
    public required string SourceName { get; init; }
    public required SourceKind SourceKind { get; init; }
    public required string Id { get; init; }
    public required string Name { get; init; }
    public string? Moniker { get; init; }
    public string? Version { get; init; }
    public string? Channel { get; init; }
    public string? MatchCriteria { get; init; }
}

public record SearchResponse
{
    public required List<SearchMatch> Matches { get; init; }
    public required List<string> Warnings { get; init; }
    public bool Truncated { get; init; }
}

public record ListMatch
{
    public required string Name { get; init; }
    public required string Id { get; init; }
    public required string LocalId { get; init; }
    public required string InstalledVersion { get; init; }
    public string? AvailableVersion { get; init; }
    public string? SourceName { get; init; }
    public string? Publisher { get; init; }
    public string? Scope { get; init; }
    public string? InstallerCategory { get; init; }
    public string? InstallLocation { get; init; }
    public List<string> PackageFamilyNames { get; init; } = [];
    public List<string> ProductCodes { get; init; } = [];
    public List<string> UpgradeCodes { get; init; } = [];
}

public record ListResponse
{
    public required List<ListMatch> Matches { get; init; }
    public required List<string> Warnings { get; init; }
    public bool Truncated { get; init; }
}

public record VersionKey
{
    public required string Version { get; init; }
    public required string Channel { get; init; }
}

public record Documentation
{
    public string? Label { get; init; }
    public required string Url { get; init; }
}

public record Installer
{
    public string? Architecture { get; init; }
    public string? InstallerType { get; init; }
    public string? Url { get; init; }
    public string? Sha256 { get; init; }
    public string? ProductCode { get; init; }
    public string? Locale { get; init; }
    public string? Scope { get; init; }
    public string? ReleaseDate { get; init; }
    public string? PackageFamilyName { get; init; }
    public string? UpgradeCode { get; init; }
    public List<string> Commands { get; init; } = [];
    public List<string> PackageDependencies { get; init; } = [];
}

public record Manifest
{
    public required string Id { get; init; }
    public required string Name { get; init; }
    public required string Version { get; init; }
    public string Channel { get; init; } = "";
    public string? Publisher { get; init; }
    public string? Description { get; init; }
    public string? Moniker { get; init; }
    public string? PackageUrl { get; init; }
    public string? PublisherUrl { get; init; }
    public string? PublisherSupportUrl { get; init; }
    public string? License { get; init; }
    public string? LicenseUrl { get; init; }
    public string? PrivacyUrl { get; init; }
    public string? Author { get; init; }
    public string? Copyright { get; init; }
    public string? CopyrightUrl { get; init; }
    public string? ReleaseNotes { get; init; }
    public string? ReleaseNotesUrl { get; init; }
    public List<string> Tags { get; init; } = [];
    public List<string> PackageDependencies { get; init; } = [];
    public List<Documentation> Documentation { get; init; } = [];
    public List<Installer> Installers { get; init; } = [];
}

public record ShowResult
{
    public required SearchMatch Package { get; init; }
    public required Manifest Manifest { get; init; }
    public Installer? SelectedInstaller { get; init; }
    public List<string> CachedFiles { get; init; } = [];
    public List<string> Warnings { get; init; } = [];
}

public record VersionsResult
{
    public required SearchMatch Package { get; init; }
    public required List<VersionKey> Versions { get; init; }
    public List<string> Warnings { get; init; } = [];
}

public record CacheWarmResult
{
    public required SearchMatch Package { get; init; }
    public List<string> CachedFiles { get; init; } = [];
    public List<string> Warnings { get; init; } = [];
}

public record SourceUpdateResult
{
    public required string Name { get; init; }
    public required SourceKind Kind { get; init; }
    public required string Detail { get; init; }
}

public record PinRecord
{
    public required string PackageId { get; init; }
    public required string Version { get; init; }
    public required string SourceId { get; init; }
    public required PinType PinType { get; init; }
}

public record InstallResult
{
    public required string PackageId { get; init; }
    public required string Version { get; init; }
    public required string InstallerPath { get; init; }
    public required string InstallerType { get; init; }
    public int ExitCode { get; init; }
    public bool Success { get; init; }
}

// Internal type for installed package tracking
internal record InstalledPackage
{
    public required string Name { get; init; }
    public required string LocalId { get; init; }
    public required string InstalledVersion { get; init; }
    public string? Publisher { get; init; }
    public string? Scope { get; init; }
    public string? InstallerCategory { get; init; }
    public string? InstallLocation { get; init; }
    public List<string> PackageFamilyNames { get; init; } = [];
    public List<string> ProductCodes { get; init; } = [];
    public List<string> UpgradeCodes { get; init; } = [];
    public SearchMatch? Correlated { get; set; }
}

internal enum SearchSemantics
{
    Many,
    Single
}

internal record LocatedMatch
{
    public required SearchMatch Display { get; init; }
    public required int SourceIndex { get; init; }
    public required MatchLocator Locator { get; init; }
}

internal abstract record MatchLocator;

internal record PreIndexedV1Locator(long PackageRowId) : MatchLocator;
internal record PreIndexedV2Locator(long PackageRowId, string PackageHash) : MatchLocator;
internal record RestLocator(string PackageId, List<VersionKey> Versions) : MatchLocator;
