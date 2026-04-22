using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using Microsoft.Data.Sqlite;

namespace WinGetCore;

internal static class InstalledPackages
{
    public static List<InstalledPackage> Collect(string? scope)
    {
        if (!RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
            throw new PlatformNotSupportedException("Installed package discovery is only supported on Windows");

        var packages = new List<InstalledPackage>();
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        bool machine = !string.Equals(scope, "user", StringComparison.OrdinalIgnoreCase);
        bool user = !string.Equals(scope, "machine", StringComparison.OrdinalIgnoreCase);

        if (machine)
        {
            CollectArpPackages(packages, seen, Microsoft.Win32.RegistryHive.LocalMachine, "Machine", "X64",
                Microsoft.Win32.RegistryView.Registry64);
            CollectArpPackages(packages, seen, Microsoft.Win32.RegistryHive.LocalMachine, "Machine", "X86",
                Microsoft.Win32.RegistryView.Registry32);
            CollectAppModelPackages(packages, seen, Microsoft.Win32.RegistryHive.LocalMachine, "Machine",
                Microsoft.Win32.RegistryView.Registry64);
        }

        if (user)
        {
            CollectArpPackages(packages, seen, Microsoft.Win32.RegistryHive.CurrentUser, "User", "X64",
                Microsoft.Win32.RegistryView.Registry64);
            CollectAppModelPackages(packages, seen, Microsoft.Win32.RegistryHive.CurrentUser, "User",
                Microsoft.Win32.RegistryView.Registry64);
        }

        return packages;
    }

    [SupportedOSPlatform("windows")]
    private static void CollectArpPackages(
        List<InstalledPackage> packages, HashSet<string> seen,
        Microsoft.Win32.RegistryHive hive, string scopeLabel, string archLabel,
        Microsoft.Win32.RegistryView view)
    {
        var arpPaths = new[]
        {
            @"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            @"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"
        };

        foreach (var arpPath in arpPaths)
        {
            var effectiveArch = arpPath.Contains("WOW6432Node") ? "X86" : archLabel;
            try
            {
                using var baseKey = Microsoft.Win32.RegistryKey.OpenBaseKey(hive, view);
                using var uninstallKey = baseKey.OpenSubKey(arpPath);
                if (uninstallKey is null) continue;

                foreach (var subkeyName in uninstallKey.GetSubKeyNames())
                {
                    try
                    {
                        using var subkey = uninstallKey.OpenSubKey(subkeyName);
                        if (subkey is null) continue;

                        var displayName = subkey.GetValue("DisplayName") as string;
                        if (string.IsNullOrWhiteSpace(displayName)) continue;

                        var systemComponent = subkey.GetValue("SystemComponent");
                        if (systemComponent is int sc && sc != 0) continue;

                        var installLocation = subkey.GetValue("InstallLocation") as string;
                        if (IsWindowsSystemPath(installLocation)) continue;

                        var version = subkey.GetValue("DisplayVersion") as string ?? "";
                        var publisher = subkey.GetValue("Publisher") as string;

                        var dedupKey = $"{displayName}|{version}|{scopeLabel}";
                        if (!seen.Add(dedupKey)) continue;

                        var productCodes = new List<string>();
                        if (LooksLikeProductCode(subkeyName))
                            productCodes.Add(subkeyName);

                        packages.Add(new InstalledPackage
                        {
                            Name = displayName,
                            LocalId = $@"ARP\{scopeLabel}\{effectiveArch}\{subkeyName}",
                            InstalledVersion = version,
                            Publisher = publisher,
                            Scope = scopeLabel,
                            InstallerCategory = effectiveArch,
                            InstallLocation = installLocation,
                            ProductCodes = productCodes,
                        });
                    }
                    catch { /* skip unreadable subkey */ }
                }
            }
            catch { /* skip unreadable hive path */ }
        }
    }

    [SupportedOSPlatform("windows")]
    private static void CollectAppModelPackages(
        List<InstalledPackage> packages, HashSet<string> seen,
        Microsoft.Win32.RegistryHive hive, string scopeLabel,
        Microsoft.Win32.RegistryView view)
    {
        // Read from StateRepository-Machine.srd (SQLite)
        var srdPath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles).Replace("Program Files", "ProgramData"),
            @"Microsoft\Windows\AppRepository\StateRepository-Machine.srd");

        // Try a more standard path
        var programData = Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData);
        srdPath = Path.Combine(programData, @"Microsoft\Windows\AppRepository\StateRepository-Machine.srd");

        if (!File.Exists(srdPath)) return;

        try
        {
            using var conn = new SqliteConnection($"Data Source={srdPath};Mode=ReadOnly");
            conn.Open();

            using var cmd = conn.CreateCommand();
            cmd.CommandText = @"
                SELECT p.PackageFullName, p.PackageFamilyName,
                       COALESCE(p.DisplayName, '') AS DisplayName,
                       COALESCE(p.Publisher, '') AS Publisher
                FROM Package p
                WHERE p.IsInbox = 0 AND p.IsFramework = 0
                ORDER BY p.PackageFullName";

            using var reader = cmd.ExecuteReader();
            while (reader.Read())
            {
                var fullName = reader.GetString(0);
                var familyName = reader.GetString(1);
                var displayName = reader.GetString(2);
                var publisher = reader.GetString(3);

                var parsed = ParseMsixFullName(fullName);
                if (parsed is null) continue;

                var dedupKey = $"MSIX|{familyName}|{parsed.Value.Version}|{scopeLabel}";
                if (!seen.Add(dedupKey)) continue;

                packages.Add(new InstalledPackage
                {
                    Name = string.IsNullOrEmpty(displayName) ? familyName : displayName,
                    LocalId = $@"MSIX\{familyName}",
                    InstalledVersion = parsed.Value.Version,
                    Publisher = string.IsNullOrEmpty(publisher) ? null : publisher,
                    Scope = scopeLabel,
                    InstallerCategory = "MSIX",
                    PackageFamilyNames = [familyName],
                });
            }
        }
        catch { /* AppModel DB not accessible */ }
    }

    private static bool IsWindowsSystemPath(string? path)
    {
        if (string.IsNullOrWhiteSpace(path)) return false;
        return path.Trim().StartsWith(@"C:\Windows\", StringComparison.OrdinalIgnoreCase);
    }

    private static bool LooksLikeProductCode(string value) =>
        value.StartsWith('{') && value.EndsWith('}');

    private static (string Version, string FamilyName)? ParseMsixFullName(string fullName)
    {
        // Format: Name_Version_Arch_ResourceId_PublisherHash
        var parts = fullName.Split('_');
        if (parts.Length < 5) return null;
        var version = parts[1];
        var publisherHash = parts[^1];
        var familyName = $"{parts[0]}_{publisherHash}";
        return (version, familyName);
    }
}
