using System.Management.Automation;
using Pinget.PowerShell.Cmdlets.Common;
using Pinget.PowerShell.Engine;
using Pinget.PowerShell.Engine.PSObjects;

namespace Pinget.PowerShell.Cmdlets.Cmdlets;

[Cmdlet(VerbsLifecycle.Uninstall, Constants.PingetNouns.Package, DefaultParameterSetName = Constants.FoundSet, SupportsShouldProcess = true)]
[Alias("uspgp")]
[OutputType(typeof(PSUninstallResult))]
public sealed class UninstallPackageCmdlet : PackageCmdlet
{
    [Parameter(ValueFromPipelineByPropertyName = true)]
    public PSPackageUninstallMode Mode { get; set; } = PSPackageUninstallMode.Default;

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public SwitchParameter Force { get; set; }

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public string? Log { get; set; }

    protected override void ProcessRecord()
    {
        var target = PSCatalogPackage?.Id ?? Id ?? Name ?? string.Join(' ', Query ?? []);
        if (!ShouldProcess(target, "Uninstall package"))
            return;

        using var client = PingetClient.Open();
        var result = client.UninstallPackage(PSCatalogPackage, Version, Id, Name, Moniker, Source, Query, Force, Log, Mode);
        foreach (var warning in result.Warnings)
            WriteWarning(warning);
        WriteObject(result.Value);
    }
}
