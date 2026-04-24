namespace Pinget.PowerShell.Engine.PSObjects;

public sealed class PSFoundCatalogPackage : PSCatalogPackage
{
    public PSFoundCatalogPackage(string id, string name, string source, string? moniker, string? version, string? match)
        : base(id, name, source, moniker)
    {
        Version = version;
        Match = match;
    }

    public string? Version { get; }

    public string? Match { get; }
}
