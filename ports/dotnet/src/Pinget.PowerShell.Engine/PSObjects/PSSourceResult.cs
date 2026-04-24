namespace Pinget.PowerShell.Engine.PSObjects;

public sealed class PSSourceResult
{
    public required string Name { get; init; }

    public required string Argument { get; init; }

    public required string Type { get; init; }

    public required string Identifier { get; init; }

    public DateTime? LastUpdate { get; init; }

    public string? SourceVersion { get; init; }
}
