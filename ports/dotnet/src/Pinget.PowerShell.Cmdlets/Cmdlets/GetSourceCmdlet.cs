using System.Management.Automation;
using Pinget.PowerShell.Cmdlets.Common;
using Pinget.PowerShell.Engine;
using Pinget.PowerShell.Engine.PSObjects;

namespace Pinget.PowerShell.Cmdlets.Cmdlets;

[Cmdlet(VerbsCommon.Get, Constants.PingetNouns.Source)]
[Alias("gpgso")]
[OutputType(typeof(PSSourceResult))]
public sealed class GetSourceCmdlet : PSCmdlet
{
    [Parameter(Position = 0, ValueFromPipeline = true, ValueFromPipelineByPropertyName = true)]
    public string? Name { get; set; }

    protected override void ProcessRecord()
    {
        using var client = PingetClient.Open();
        var result = client.GetSources(Name);
        WriteObject(result.Items, enumerateCollection: true);
    }
}
