# Pinget agent guide

This directory is being prepared to become its own `pinget` repository. Treat `pinget\` as the effective repo root when making changes here.

## Scope

- Maintain the Rust CLI + core in `rust\`
- Maintain the C# CLI + core + PowerShell module in `dotnet\`
- Keep behavior aligned with WinGet where practical
- Keep the implementation **COM-free**

## Hard exclusions

Do not add or depend on:

- COM / WinRT / `Microsoft.Management.Deployment`
- DSC / `configure` / `dscv3` / `Microsoft.WinGet.Configuration`
- `mcp`

If an upstream behavior depends on one of those, document the limit instead of faking native integration.

## Working conventions

1. Prefer subtree-relative paths in docs, scripts, and instructions so the directory can be extracted cleanly later.
2. Keep Pinget-specific documentation inside this directory when possible.
3. Preserve the existing branding split:
   - product name: `Pinget`
   - C# namespaces/assemblies: `Pinget.*`
   - Rust crates/binaries: `pinget-*` / `pinget`
4. Keep Rust and C# behavior aligned when the same feature exists in both implementations.

## Validation

Run the existing toolchain checks that apply to the files you changed.

### Rust

```powershell
cargo +nightly fmt --manifest-path rust\Cargo.toml --all
cargo clippy -q --manifest-path rust\Cargo.toml --workspace --tests -- -D warnings
cargo test -p pinget-core --manifest-path rust\Cargo.toml
cargo test -p pinget-cli --manifest-path rust\Cargo.toml
cargo build -p pinget-cli --manifest-path rust\Cargo.toml
```

### C#

```powershell
dotnet format dotnet\WinGetDotNet.slnx
dotnet build dotnet\WinGetDotNet.slnx -c Release
dotnet test dotnet\src\WinGetCore.Tests\WinGetCore.Tests.csproj -c Release
pwsh -NoLogo -NoProfile -File (Resolve-Path 'dotnet\tests\RunTests.ps1')
```

## Migration prep guidance

When adding new files or build instructions, prefer layouts that will still make sense after this directory is copied into a standalone repository:

- avoid parent-repo-relative paths
- keep Pinget docs close to the subtree
- avoid coupling to unrelated `winget-cli` infrastructure unless there is no practical alternative
