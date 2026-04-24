# WinGet ports

This directory contains two non-native WinGet client ports that reuse WinGet-style source, manifest, cache, and package-selection logic without using the WinGet COM API.

## Layout

| Path | Language | Output | Notes |
| --- | --- | --- | --- |
| `ports\rust` | Rust | `winget` CLI + `winget-core` library | Cargo workspace with a reusable core crate and a CLI crate |
| `ports\dotnet` | C# / .NET 10 | `winget` CLI + `WinGetCore` library | Solution with a reusable core library, CLI app, and tests |

## C# assembly and namespace structure

| Project | Assembly | Primary namespace | Role |
| --- | --- | --- | --- |
| `ports\dotnet\src\WinGetCore\WinGetCore.csproj` | `WinGetCore.dll` | `WinGetCore` | Core library |
| `ports\dotnet\src\WinGetCli\WinGetCli.csproj` | `winget.dll` | top-level statements in `Program.cs` | CLI front end |
| `ports\dotnet\src\WinGetCore.Tests\WinGetCore.Tests.csproj` | `WinGetCore.Tests.dll` | `WinGetCore.Tests` | Tests |

## Current scope

Both ports currently support a substantial WinGet-like subset, including:

- `search`, `show`, `list`, `upgrade`
- `source list`, `source update`, `source export`, `source add`, `source remove`, `source reset`
- `cache warm`
- `download`, `hash`, `validate`, `export`, `error`, `settings export`, `features`
- `pin list`, `pin add`, `pin remove`, `pin reset`
- `install`, `uninstall`, `import`

Structured manifest output is also supported:

- `show --output json|yaml`
- `search --manifests --output json|yaml`

## Portability model

These ports are designed to keep **source-backed functionality** working cross-platform where practical.

- Commands like `search`, `show`, `cache warm`, `download`, `source`, and manifest shaping are intended to work on Windows and Linux.
- On Linux, **installed-state and package-action behavior is best-effort**:
  - `list` and upgrade inventory return empty results with an unsupported warning
  - `install`, `uninstall`, executed `upgrade`, and non-dry-run `import` return explicit no-op results with unsupported warnings

## Custom REST sources

Both ports support custom REST sources, including third-party services such as `winget.pro`.

The CLI accepts both:

1. The earlier positional form:

```powershell
winget source add winget.pro https://api.example.test/feed --type Microsoft.Rest
```

2. The upstream-style option form:

```powershell
winget source add -n winget.pro -a https://api.example.test/feed -t Microsoft.Rest --trust-level trusted
```

Notes:

- `Microsoft.Rest` maps to the existing REST source kind in both ports.
- `Microsoft.PreIndexed.Package` maps to the preindexed source kind.
- `--trust-level` is currently accepted for CLI compatibility but is a no-op; it is not yet persisted or enforced.

## Build and test

### Rust

```powershell
cargo test -p winget-core --manifest-path ports\rust\Cargo.toml
cargo build -p winget-cli --release --manifest-path ports\rust\Cargo.toml
```

Run:

```powershell
cargo run -p winget-cli --manifest-path ports\rust\Cargo.toml -- search WinMerge
```

### C#

```powershell
dotnet test ports\dotnet\src\WinGetCore.Tests\WinGetCore.Tests.csproj -c Release
dotnet build ports\dotnet\src\WinGetCli\WinGetCli.csproj -c Release
```

Run:

```powershell
dotnet run --project ports\dotnet\src\WinGetCli\WinGetCli.csproj -- search WinMerge
```

## Repository structure

### Rust

- `ports\rust\crates\winget-core` - core repository, source, manifest, and action logic
- `ports\rust\crates\winget-cli` - CLI wrapper

### C#

- `ports\dotnet\src\WinGetCore` - core library
- `ports\dotnet\src\WinGetCli` - CLI wrapper
- `ports\dotnet\src\WinGetCore.Tests` - tests

## Status

These ports are best treated as **experimental WinGet-compatible implementations** focused on:

- source-backed package discovery
- manifest retrieval and shaping
- custom REST source support
- reusable library surfaces in Rust and C#

They are not intended to be drop-in, fully complete replacements for the native Windows Package Manager client.
