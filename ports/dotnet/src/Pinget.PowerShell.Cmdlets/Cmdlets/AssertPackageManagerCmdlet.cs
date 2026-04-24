using System.Management.Automation;
using Pinget.PowerShell.Cmdlets.Common;
using Pinget.PowerShell.Engine;

namespace Pinget.PowerShell.Cmdlets.Cmdlets;

[Cmdlet(VerbsLifecycle.Assert, Constants.PingetNouns.PingetPackageManager, DefaultParameterSetName = Constants.IntegrityVersionSet)]
[Alias("apgpm")]
public sealed class AssertPackageManagerCmdlet : PingetPackageManagerCmdlet
{
    protected override void ProcessRecord()
    {
        using var client = PingetClient.Open();
        client.AssertPackageManager(ParameterSetName == Constants.IntegrityLatestSet ? null : Version, Latest, IncludePrerelease);
    }
}
