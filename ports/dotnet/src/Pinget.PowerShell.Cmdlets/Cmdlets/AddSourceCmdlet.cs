using System.Management.Automation;
using Pinget.PowerShell.Cmdlets.Common;
using Pinget.PowerShell.Engine;
using Pinget.PowerShell.Engine.PSObjects;

namespace Pinget.PowerShell.Cmdlets.Cmdlets;

[Cmdlet(VerbsCommon.Add, Constants.PingetNouns.Source)]
[Alias("apgs")]
public sealed class AddSourceCmdlet : PSCmdlet
{
    [Parameter(Mandatory = true, ValueFromPipeline = true, ValueFromPipelineByPropertyName = true)]
    public string Name { get; set; } = string.Empty;

    [Parameter(Mandatory = true, ValueFromPipeline = true, ValueFromPipelineByPropertyName = true)]
    public string Argument { get; set; } = string.Empty;

    [Parameter(ValueFromPipeline = true, ValueFromPipelineByPropertyName = true)]
    [ValidateSet("Microsoft.Rest", "Microsoft.PreIndexed.Package")]
    public string? Type { get; set; }

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public PSSourceTrustLevel TrustLevel { get; set; } = PSSourceTrustLevel.Default;

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public SwitchParameter Explicit { get; set; }

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public int Priority { get; set; }

    protected override void ProcessRecord()
    {
        using var client = PingetClient.Open();
        client.AddSource(Name, Argument, Type, TrustLevel, Explicit, Priority);
    }
}
