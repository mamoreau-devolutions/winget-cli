using System.Management.Automation;
using Pinget.PowerShell.Engine.PSObjects;

namespace Pinget.PowerShell.Cmdlets.Common;

public abstract class InstallerSelectionCmdlet : PackageCmdlet
{
    [Parameter(ValueFromPipelineByPropertyName = true)]
    public SwitchParameter AllowHashMismatch { get; set; }

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public PSProcessorArchitecture Architecture { get; set; } = PSProcessorArchitecture.Default;

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public PSPackageInstallerType InstallerType { get; set; } = PSPackageInstallerType.Default;

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public string? Locale { get; set; }

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public PSPackageInstallScope Scope { get; set; } = PSPackageInstallScope.Any;

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public SwitchParameter SkipDependencies { get; set; }
}
