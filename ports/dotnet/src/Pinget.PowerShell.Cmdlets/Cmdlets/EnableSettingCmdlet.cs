using System.Management.Automation;
using Pinget.PowerShell.Cmdlets.Common;
using Pinget.PowerShell.Engine;

namespace Pinget.PowerShell.Cmdlets.Cmdlets;

[Cmdlet(VerbsLifecycle.Enable, Constants.PingetNouns.Setting)]
[Alias("epgs")]
public sealed class EnableSettingCmdlet : PSCmdlet
{
    [Parameter(Position = 0, Mandatory = true, ValueFromPipeline = true, ValueFromPipelineByPropertyName = true)]
    [ValidateSet(
        "LocalManifestFiles",
        "BypassCertificatePinningForMicrosoftStore",
        "InstallerHashOverride",
        "LocalArchiveMalwareScanOverride",
        "ProxyCommandLineOptions")]
    public string Name { get; set; } = string.Empty;

    protected override void ProcessRecord()
    {
        using var client = PingetClient.Open();
        client.SetAdminSetting(Name, true);
    }
}
