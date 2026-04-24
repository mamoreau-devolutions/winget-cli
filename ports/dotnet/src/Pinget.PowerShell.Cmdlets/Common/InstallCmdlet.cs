using System.Management.Automation;
using Pinget.PowerShell.Engine.PSObjects;

namespace Pinget.PowerShell.Cmdlets.Common;

public abstract class InstallCmdlet : InstallerSelectionCmdlet
{
    private string? _location;

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public PSPackageInstallMode Mode { get; set; } = PSPackageInstallMode.Default;

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public string? Override { get; set; }

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public string? Custom { get; set; }

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public string? Location
    {
        get => _location;
        set
        {
            if (string.IsNullOrWhiteSpace(value))
            {
                _location = value;
                return;
            }

            _location = Path.IsPathRooted(value)
                ? value
                : Path.Combine(SessionState.Path.CurrentFileSystemLocation.Path, value);
        }
    }

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public string? Log { get; set; }

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public SwitchParameter Force { get; set; }

    [Parameter(ValueFromPipelineByPropertyName = true)]
    public string? Header { get; set; }
}
