using System.Text.Json;
using System.Text.Json.Serialization;

namespace WinGetCore;

internal record SourceStore
{
    public List<SourceRecord> Sources { get; set; } = [];

    public static SourceStore Default() => new()
    {
        Sources =
        [
            new SourceRecord
            {
                Name = "winget",
                Kind = SourceKind.PreIndexed,
                Arg = "https://cdn.winget.microsoft.com/cache",
                Identifier = "Microsoft.Winget.Source_8wekyb3d8bbwe",
            },
            new SourceRecord
            {
                Name = "msstore",
                Kind = SourceKind.Rest,
                Arg = "https://storeedgefd.dsx.mp.microsoft.com/v9.0",
                Identifier = "StoreEdgeFD",
            }
        ]
    };
}

[JsonSerializable(typeof(SourceStore))]
[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase, WriteIndented = true)]
internal partial class SourceStoreContext : JsonSerializerContext;

internal static class SourceStoreManager
{
    private static string AppRoot()
    {
        var localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        return Path.Combine(localAppData, "winget-dotnet");
    }

    public static void EnsureAppDirs()
    {
        var root = AppRoot();
        Directory.CreateDirectory(root);
        Directory.CreateDirectory(Path.Combine(root, "sources"));
    }

    public static string SourceStateDir(SourceRecord source)
    {
        var safeName = string.Concat(source.Name.Select(c =>
            @"\/:*?""<>|".Contains(c) ? '_' : c));
        return Path.Combine(AppRoot(), "sources", safeName);
    }

    public static string PinsDbPath() => Path.Combine(AppRoot(), "pins.db");

    public static SourceStore Load()
    {
        var path = Path.Combine(AppRoot(), "sources.json");
        if (!File.Exists(path))
            return SourceStore.Default();

        var json = File.ReadAllText(path);
        return JsonSerializer.Deserialize(json, SourceStoreContext.Default.SourceStore) ?? SourceStore.Default();
    }

    public static void Save(SourceStore store)
    {
        var path = Path.Combine(AppRoot(), "sources.json");
        var json = JsonSerializer.Serialize(store, SourceStoreContext.Default.SourceStore);
        File.WriteAllText(path, json);
    }
}
