using System.Diagnostics;
using System.IO.Compression;
using System.Runtime.InteropServices;

namespace WinGetCore;

internal static class InstallerDispatch
{
    public static int Execute(string installerPath, string installerType, bool silent, Installer installer)
    {
        if (!RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
            throw new PlatformNotSupportedException("Installing packages is only supported on Windows");

        return installerType.ToLowerInvariant() switch
        {
            "msi" or "wix" => RunMsi(installerPath, silent),
            "msix" or "appx" => RunMsix(installerPath),
            "zip" => ExtractZip(installerPath),
            _ => RunExe(installerPath, silent, installer)
        };
    }

    public static int Uninstall(string packageId, bool silent)
    {
        if (!RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
            throw new PlatformNotSupportedException("Uninstalling packages is only supported on Windows");

        // Search ARP registry for uninstall command
        var hives = new[]
        {
            (Microsoft.Win32.RegistryHive.LocalMachine, Microsoft.Win32.RegistryView.Registry64),
            (Microsoft.Win32.RegistryHive.LocalMachine, Microsoft.Win32.RegistryView.Registry32),
            (Microsoft.Win32.RegistryHive.CurrentUser, Microsoft.Win32.RegistryView.Registry64),
        };

        var arpPaths = new[]
        {
            @"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            @"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        };

        foreach (var (hive, view) in hives)
        {
            using var baseKey = Microsoft.Win32.RegistryKey.OpenBaseKey(hive, view);
            foreach (var arpPath in arpPaths)
            {
                using var key = baseKey.OpenSubKey(arpPath);
                if (key is null) continue;

                foreach (var subkeyName in key.GetSubKeyNames())
                {
                    using var subkey = key.OpenSubKey(subkeyName);
                    if (subkey is null) continue;

                    var displayName = subkey.GetValue("DisplayName") as string ?? "";
                    if (!subkeyName.Equals(packageId, StringComparison.OrdinalIgnoreCase) &&
                        !displayName.Equals(packageId, StringComparison.OrdinalIgnoreCase))
                        continue;

                    // Found it
                    var uninstallCmd = subkey.GetValue("QuietUninstallString") as string
                        ?? subkey.GetValue("UninstallString") as string
                        ?? throw new InvalidOperationException("No uninstall command found in registry");

                    var psi = new ProcessStartInfo("cmd", $"/C {uninstallCmd}")
                    {
                        UseShellExecute = false,
                    };
                    if (silent && !uninstallCmd.Contains("/S") && !uninstallCmd.Contains("/quiet"))
                    {
                        psi.Arguments = $"/C {uninstallCmd} /S";
                    }

                    using var proc = Process.Start(psi) ?? throw new InvalidOperationException("Failed to start uninstaller");
                    proc.WaitForExit();
                    return proc.ExitCode;
                }
            }
        }

        // Fallback: try MSIX removal
        var msixPsi = new ProcessStartInfo("powershell", $"-NoProfile -Command \"Get-AppxPackage -Name '*{packageId}*' | Remove-AppxPackage\"")
        {
            UseShellExecute = false,
        };
        using var msixProc = Process.Start(msixPsi) ?? throw new InvalidOperationException("Failed to start Remove-AppxPackage");
        msixProc.WaitForExit();
        return msixProc.ExitCode;
    }

    private static int RunMsi(string path, bool silent)
    {
        var args = silent ? $"/i \"{path}\" /quiet /norestart" : $"/i \"{path}\" /passive /norestart";
        var psi = new ProcessStartInfo("msiexec", args) { UseShellExecute = false };
        using var proc = Process.Start(psi) ?? throw new InvalidOperationException("Failed to run msiexec");
        proc.WaitForExit();
        return proc.ExitCode;
    }

    private static int RunMsix(string path)
    {
        var psi = new ProcessStartInfo("powershell", $"-NoProfile -Command \"Add-AppxPackage -Path '{path}'\"")
        {
            UseShellExecute = false
        };
        using var proc = Process.Start(psi) ?? throw new InvalidOperationException("Failed to run Add-AppxPackage");
        proc.WaitForExit();
        return proc.ExitCode;
    }

    private static int ExtractZip(string path)
    {
        var target = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "Programs");
        Directory.CreateDirectory(target);
        ZipFile.ExtractToDirectory(path, target, overwriteFiles: true);
        return 0;
    }

    private static int RunExe(string path, bool silent, Installer installer)
    {
        var psi = new ProcessStartInfo(path) { UseShellExecute = false };
        if (silent)
        {
            psi.ArgumentList.Add("/S");
            psi.ArgumentList.Add("/SILENT");
            psi.ArgumentList.Add("/VERYSILENT");
        }
        using var proc = Process.Start(psi) ?? throw new InvalidOperationException("Failed to run installer");
        proc.WaitForExit();
        return proc.ExitCode;
    }
}
