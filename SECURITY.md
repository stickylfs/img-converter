# Security Policy

## Reporting a vulnerability

Please report security issues privately through **GitHub Security Advisories** ("Report a vulnerability" in the *Security* tab of this repository) instead of opening a public issue. You will receive an answer as soon as possible.

Malformed images that crash or hang the converter are welcome reports; please attach the file if you can share it.

## Supported versions

Only the latest release receives fixes.

## Threat model

Every file selected in Explorer is treated as untrusted input.

- **Explorer isolation**: `img_converter_shell.dll` (loaded into `explorer.exe`) only reads file names and the configuration. All decoding happens in the separate `img-converter.exe` process, so a malicious or corrupt file cannot crash Explorer.
- **Decompression bombs**: before pixel buffers are allocated, inputs are limited to 100 MB on disk, 16,384 px per side, 268 megapixels and 512 MB of decoded pixels. PDF pages that would exceed these limits are rendered at a lower resolution.
- **No command injection**: the worker is started with `CreateProcessW` and an explicit executable path, never through a shell. Very long selections are passed in a temporary list file.
- **Safe writes**: output names are reserved atomically (`O_CREAT | O_EXCL`), data is written to a temporary file in the destination folder and renamed into place, so concurrent conversions cannot overwrite each other and failures leave no partial files.
- **Release hardening**: release builds use `panic = "abort"`.

## Privacy

- No network access: the program contains no networking code and sends no telemetry.
- Images never leave your computer. The only files written besides the converted images are `config.ini` and `last_error.log` in `%LOCALAPPDATA%\img-converter`.
- No administrator rights are required; all registry changes are made under `HKEY_CURRENT_USER` and removed on uninstall.
