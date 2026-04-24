# Pinget ports

This directory contains Devolutions **Pinget** ports: non-native, portable WinGet-compatible implementations that reuse WinGet-style source, manifest, cache, and package-selection logic without using the WinGet COM API.

## Layout

| Path | Language | Output | Notes |
| --- | --- | --- | --- |
| `ports\rust` | Rust | `pinget` CLI + `pinget-core` library | Cargo workspace with a reusable core crate and a CLI crate |
| `ports\dotnet` | C# / .NET 10 | `pinget` CLI + `Pinget.Core` library + Pinget PowerShell module | Solution with a reusable core library, CLI app, tests, and a PowerShell engine/cmdlet layer |

## C# assembly and namespace structure

| Project | Assembly | Primary namespace | Role |
| --- | --- | --- | --- |
| `ports\dotnet\src\WinGetCore\WinGetCore.csproj` | `Pinget.Core.dll` | `Pinget.Core` | Core library |
| `ports\dotnet\src\WinGetCli\WinGetCli.csproj` | `pinget.dll` | `Pinget.Cli` (top-level statements in `Program.cs`) | CLI front end |
| `ports\dotnet\src\WinGetCore.Tests\WinGetCore.Tests.csproj` | `Pinget.Core.Tests.dll` | `Pinget.Core.Tests` | Tests |
| `ports\dotnet\src\Pinget.PowerShell.Engine\Pinget.PowerShell.Engine.csproj` | `Pinget.PowerShell.Engine.dll` | `Pinget.PowerShell.Engine` | PowerShell engine layer over `Pinget.Core` |
| `ports\dotnet\src\Pinget.PowerShell.Cmdlets\Pinget.PowerShell.Cmdlets.csproj` | `Pinget.PowerShell.Cmdlets.dll` | `Pinget.PowerShell.Cmdlets` | Renamed PowerShell cmdlet surface |

## Current scope

Both CLI ports currently support a substantial WinGet-like subset, including:

- `search`, `show`, `list`, `upgrade`
- `source list`, `source update`, `source export`, `source add`, `source edit`, `source remove`, `source reset`
- `cache warm`
- `download`, `hash`, `validate`, `export`, `error`, `settings export`, `settings set`, `settings reset`, `features`
- `pin list`, `pin add`, `pin remove`, `pin reset`
- `install`, `uninstall`, `repair`, `import`

Structured manifest output is also supported:

- `show --output json|yaml`
- `search --manifests --output json|yaml`

The C# port also ships a **Pinget PowerShell module** that mirrors the upstream `Microsoft.WinGet.Client` cmdlet family with renamed `Pinget` nouns, backed entirely by `Pinget.Core` rather than COM/WinRT APIs.

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
pinget source add winget.pro https://api.example.test/feed --type Microsoft.Rest
```

2. The upstream-style option form:

```powershell
pinget source add -n winget.pro -a https://api.example.test/feed -t Microsoft.Rest --trust-level trusted
```

Notes:

- `Microsoft.Rest` maps to the existing REST source kind in both ports.
- `Microsoft.PreIndexed.Package` maps to the preindexed source kind.
- In both ports, `--trust-level`, `--explicit`, and source priority metadata are now persisted as source metadata and influence source selection behavior.

## Build and test

### Rust

```powershell
cargo test -p pinget-core --manifest-path ports\rust\Cargo.toml
cargo build -p pinget-cli --release --manifest-path ports\rust\Cargo.toml
```

Run:

```powershell
cargo run -p pinget-cli --manifest-path ports\rust\Cargo.toml -- search WinMerge
```

### C#

```powershell
dotnet test ports\dotnet\src\WinGetCore.Tests\WinGetCore.Tests.csproj -c Release
dotnet build ports\dotnet\src\WinGetCli\WinGetCli.csproj -c Release
pwsh -NoLogo -NoProfile -File (Resolve-Path 'ports\dotnet\tests\RunTests.ps1')
```

Run:

```powershell
dotnet run --project ports\dotnet\src\WinGetCli\WinGetCli.csproj -- search WinMerge
```

## Repository structure

### Rust

- `ports\rust\crates\winget-core` - `pinget-core` library source
- `ports\rust\crates\winget-cli` - `pinget` CLI wrapper

### C#

- `ports\dotnet\src\WinGetCore` - `Pinget.Core` library
- `ports\dotnet\src\WinGetCli` - `Pinget.Cli` wrapper
- `ports\dotnet\src\WinGetCore.Tests` - tests
- `ports\dotnet\src\Pinget.PowerShell.Engine` - PowerShell engine over the C# core
- `ports\dotnet\src\Pinget.PowerShell.Cmdlets` - PowerShell cmdlet implementation
- `ports\dotnet\tests` - Pinget PowerShell Pester coverage

## Status

These ports are best treated as **experimental Pinget / portable winget implementations** focused on:

- source-backed package discovery
- manifest retrieval and shaping
- custom REST source support
- reusable library surfaces in Rust and C#
- a COM-free PowerShell automation surface over the C# port

They intentionally exclude COM/WinRT APIs, DSC/configuration flows (`configure`, `dscv3`, `Microsoft.WinGet.Configuration`), and `mcp`, so they are not intended to be drop-in, fully complete replacements for the native Windows Package Manager client.
