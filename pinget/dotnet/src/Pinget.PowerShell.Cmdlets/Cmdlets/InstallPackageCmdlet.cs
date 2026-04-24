using System.Management.Automation;
using Pinget.PowerShell.Cmdlets.Common;
using Pinget.PowerShell.Engine;
using Pinget.PowerShell.Engine.PSObjects;

namespace Pinget.PowerShell.Cmdlets.Cmdlets;

[Cmdlet(VerbsLifecycle.Install, Constants.PingetNouns.Package, DefaultParameterSetName = Constants.FoundSet, SupportsShouldProcess = true)]
[Alias("ispgp")]
[OutputType(typeof(PSInstallResult))]
public sealed class InstallPackageCmdlet : InstallCmdlet
{
    protected override void ProcessRecord()
    {
        var target = PSCatalogPackage?.Id ?? Id ?? Name ?? string.Join(' ', Query ?? []);
        if (!ShouldProcess(target, "Install package"))
            return;

        using var client = PingetClient.Open();
        var result = client.InstallPackage(
            PSCatalogPackage,
            Version,
            Id,
            Name,
            Moniker,
            Source,
            Query,
            AllowHashMismatch,
            Override,
            Custom,
            Location,
            Log,
            Force,
            Header,
            SkipDependencies,
            Locale,
            Scope,
            Architecture,
            Mode,
            InstallerType);

        foreach (var warning in result.Warnings)
            WriteWarning(warning);
        WriteObject(result.Value);
    }
}
