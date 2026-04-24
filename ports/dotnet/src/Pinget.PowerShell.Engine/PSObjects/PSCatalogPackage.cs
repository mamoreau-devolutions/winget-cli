namespace Pinget.PowerShell.Engine.PSObjects;

public abstract class PSCatalogPackage
{
    protected PSCatalogPackage(string id, string name, string source, string? moniker)
    {
        Id = id;
        Name = name;
        Source = source;
        Moniker = moniker;
    }

    public string Id { get; }

    public string Name { get; }

    public string Source { get; }

    public string? Moniker { get; }
}
