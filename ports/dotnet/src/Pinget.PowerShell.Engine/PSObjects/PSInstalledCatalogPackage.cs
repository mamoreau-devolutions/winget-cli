namespace Pinget.PowerShell.Engine.PSObjects;

public sealed class PSInstalledCatalogPackage : PSCatalogPackage
{
    public PSInstalledCatalogPackage(
        string id,
        string name,
        string source,
        string? moniker,
        string installedVersion,
        IReadOnlyList<string> availableVersions,
        string? publisher,
        string? scope)
        : base(id, name, source, moniker)
    {
        InstalledVersion = installedVersion;
        AvailableVersions = availableVersions;
        Publisher = publisher;
        Scope = scope;
    }

    public string InstalledVersion { get; }

    public IReadOnlyList<string> AvailableVersions { get; }

    public bool IsUpdateAvailable => AvailableVersions.Count > 0;

    public string? Publisher { get; }

    public string? Scope { get; }
}
