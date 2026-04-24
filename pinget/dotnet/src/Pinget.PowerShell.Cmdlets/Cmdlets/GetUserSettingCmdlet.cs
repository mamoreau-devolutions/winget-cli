using System.Management.Automation;
using Pinget.PowerShell.Cmdlets.Common;
using Pinget.PowerShell.Engine;

namespace Pinget.PowerShell.Cmdlets.Cmdlets;

[Cmdlet(VerbsCommon.Get, Constants.PingetNouns.UserSetting)]
[Alias("gpgus", "Get-PingetUserSettings")]
[OutputType(typeof(System.Collections.Hashtable))]
public sealed class GetUserSettingCmdlet : PSCmdlet
{
    protected override void ProcessRecord()
    {
        using var client = PingetClient.Open();
        WriteObject(HashtableConverter.ToHashtable(client.GetUserSettings()));
    }
}
