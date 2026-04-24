namespace Pinget.PowerShell.Engine.PSObjects;

public sealed class PSDownloadResult
{
    public required string Id { get; init; }

    public required string Name { get; init; }

    public required string Source { get; init; }

    public required string Version { get; init; }

    public required string DownloadDirectory { get; init; }

    public required string DownloadedInstallerPath { get; init; }
}
