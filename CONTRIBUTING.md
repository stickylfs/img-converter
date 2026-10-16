# Contributing to img-converter

Thank you for your interest in contributing! Bug reports, new formats, translations, fixes and documentation improvements are all welcome. Read [docs/architecture.md](docs/architecture.md) first for an overview of the crates.

## Development setup

### Prerequisites

- Windows 10 or 11
- [Rust](https://rustup.rs/) 1.82 or newer
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the *Desktop development with C++* workload (MSVC linker and Windows SDK)

### Build and test

```bash
git clone https://github.com/<your-username>/img-converter.git
cd img-converter

cargo test --workspace                 # run all test suites
cargo build --workspace                # debug build

# Release installer: it embeds the worker and the shell DLL, so build those first.
cargo build --release -p img-converter-worker -p img-converter-shell
cargo build --release -p img-converter-installer
```

The installer is written to `target/release/img-converter-setup.exe`. Building the installer before its payload prints a warning and produces an installer that refuses to run.

Some tests exercise optional Windows codecs (AVIF, HEIC) and skip those checks when the codec is not installed.

### Trying the shell extension

Install a build with the generated installer. Explorer keeps the DLL loaded, so after rebuilding run the installer again and choose **Repair** (it renames the locked DLL out of the way); restarting Explorer picks up the new version.

## Adding a format

1. Add the variant and its extensions, MIME type and alpha support to `crates/img-converter-common/src/format.rs` (including magic-byte detection).
2. Add the allowed conversions in `crates/img-converter-common/src/matrix.rs`.
3. Implement `ImageDecoder` and/or `ImageEncoder` in `crates/img-converter-core/src/codecs/` and wire it in `engine.rs`.
4. Register the extension in `SHELL_CLASSES` in `crates/img-converter-installer/src/main.rs`.
5. Add tests in `crates/img-converter-core/tests/` and update the format table in `README.md`.

## Pull request checklist

1. Format the code: `cargo fmt --all`
2. No lint warnings: `cargo clippy --workspace --all-targets -- -D warnings`
3. Tests pass: `cargo test --workspace`
4. Update `CHANGELOG.md` for user-visible changes.
5. Describe what the change does and why in the pull request.

## Code style

- The shell extension runs inside `explorer.exe`: it must never decode images, panic or block. Keep its dependencies to `img-converter-common` and `windows`.
- Prefer permissively licensed (MIT/Apache/BSD) dependencies; check new ones with `cargo tree` and update `THIRD-PARTY-NOTICES`.
- User-facing text must be available in English and Spanish. A new kind of failure gets a `FailureKind` and a plain-language explanation in `i18n.rs` (`failure_reason`).

## Releasing

1. Update the version in the workspace `Cargo.toml` and add a section to `CHANGELOG.md`.
2. Tag the commit (`git tag v1.2.3 && git push --tags`).
3. The **Release** workflow builds the x64, x86 and ARM64 installers, generates `SHA256SUMS.txt` and publishes a GitHub release.

## License

By contributing, you agree that your contributions are licensed under the MIT License, like the rest of the project.
