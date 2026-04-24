using System.Management.Automation;
using Pinget.PowerShell.Cmdlets.Common;
using Pinget.PowerShell.Engine;

namespace Pinget.PowerShell.Cmdlets.Cmdlets;

[Cmdlet(VerbsCommon.Reset, Constants.PingetNouns.Source, DefaultParameterSetName = Constants.DefaultSet)]
[Alias("rspgs")]
public sealed class ResetSourceCmdlet : PSCmdlet
{
    [Parameter(
        Position = 0,
        Mandatory = true,
        ParameterSetName = Constants.DefaultSet,
        ValueFromPipeline = true,
        ValueFromPipelineByPropertyName = true)]
    public string? Name { get; set; }

    [Parameter(ParameterSetName = Constants.OptionalSet, ValueFromPipelineByPropertyName = true)]
    public SwitchParameter All { get; set; }

    protected override void ProcessRecord()
    {
        using var client = PingetClient.Open();
        client.ResetSource(Name, All);
    }
}
