using System.Management.Automation;
using Pinget.PowerShell.Cmdlets.Common;
using Pinget.PowerShell.Engine;

namespace Pinget.PowerShell.Cmdlets.Cmdlets;

[Cmdlet(VerbsCommon.Get, Constants.PingetNouns.Version)]
[Alias("gpgv")]
[OutputType(typeof(string))]
public sealed class GetVersionCmdlet : PSCmdlet
{
    protected override void ProcessRecord()
    {
        using var client = PingetClient.Open();
        WriteObject(client.GetVersion());
    }
}
