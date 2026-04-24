using System.Collections;
using System.Management.Automation;
using Pinget.PowerShell.Cmdlets.Common;
using Pinget.PowerShell.Engine;

namespace Pinget.PowerShell.Cmdlets.Cmdlets;

[Cmdlet(VerbsCommon.Set, Constants.PingetNouns.UserSetting)]
[Alias("spgus", "Set-PingetUserSettings")]
[OutputType(typeof(Hashtable))]
public sealed class SetUserSettingCmdlet : PSCmdlet
{
    [Parameter(Mandatory = true, ValueFromPipelineByPropertyName = true)]
    public Hashtable UserSettings { get; set; } = new();

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public SwitchParameter Merge { get; set; }

    protected override void ProcessRecord()
    {
        using var client = PingetClient.Open();
        var result = client.SetUserSettings(HashtableConverter.ToJsonObject(UserSettings), Merge);
        WriteObject(HashtableConverter.ToHashtable(result));
    }
}
