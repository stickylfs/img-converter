# Architecture

img-converter is split into small crates so that the code loaded into
`explorer.exe` stays tiny and never touches image data.

```
 File Explorer (explorer.exe)
   │  right-click: IContextMenu (classic menu, also "Show more options" on Windows 11)
   ▼
 img_converter_shell.dll ── reads config.ini, builds the "Convert" submenu
   │  CreateProcessW("img-converter.exe --notify <format> <files...>")
   ▼
 img-converter.exe ── decodes, converts and writes the files, then exits
```

## Crates

| Crate | Output | Role |
| :--- | :--- | :--- |
| `img-converter-common` | library | `ImageFormat`, conversion matrix, `config.ini` parser, policies, limits, i18n, WIC codec detection |
| `img-converter-core` | library | Decoders/encoders, PDF rendering and generation, EXIF orientation, transparency, atomic writes, batch engine |
| `img-converter-worker` | `img-converter.exe` | Command-line entry point used by the shell extension and by scripts |
| `img-converter-shell` | `img_converter_shell.dll` | COM in-process server implementing the context menu |
| `img-converter-installer` | `img-converter-setup.exe` | Embeds the worker, the DLL and the icon; installs per user and uninstalls |

## Shell extension

- Registered per user under `HKCU\Software\Classes\CLSID\{3CA619D1-34F6-42E1-B0E3-7AE990A32001}` with `ThreadingModel = Apartment`, and as a `ContextMenuHandlers` entry for every supported extension and common PDF ProgIDs.
- Implements `IShellExtInit` + `IContextMenu3` (classic menu). An `IExplorerCommand` implementation is included as well, but the Windows 11 compact menu only shows it for packaged apps (MSIX / sparse package with a trusted signature), so it is not registered there; Windows 11 users reach the menu through **Show more options**.
- Reads `config.ini` on every menu invocation, so configuration changes apply immediately.
- Locates `img-converter.exe` next to its own DLL and starts it with an explicit application name. Selections that do not fit in a command line (32,767 characters) are passed in a temporary UTF-8 list file (`--list`).

## Conversion pipeline (`img-converter-core`)

For each file (up to four in parallel):

1. **Read** the file after checking its size against `DecodeLimits`.
2. **Detect** the format from its magic bytes; the extension disambiguates TIFF-based camera RAWs and `.jfif`.
3. **Decode** with the matching codec. Dimension and memory limits are enforced from the image header before allocating pixels. HEIC, AVIF and camera RAW use Windows codecs through WIC (RAW falls back to the JPEG preview embedded by the camera); PDF pages are rendered with `Windows.Data.Pdf`.
4. **Prepare**: drop an all-opaque alpha channel, apply the EXIF orientation (and reset the tag), composite transparency onto the matte color when the target has no alpha.
5. **Encode** with the target codec, embedding ICC/EXIF metadata when enabled and supported.
6. **Write**: reserve the output name atomically (`photo.webp`, `photo (1).webp`, ...), write a temporary file in the same folder, `fsync` it and rename it into place.

Special cases:

- **Several files → PDF** are merged into one document (natural file-name order, PDF inputs contribute all their pages), named after the first file.
- **PDF → image** renders every page; multi-page documents go to a `<name>` subfolder as `<name>_page_01.png`, ...
- **`.jfif` → JPG** is a lossless copy.

The worker writes failures to `%LOCALAPPDATA%\img-converter\last_error.log`, shows a summary dialog when started with `--notify`, and returns an exit code (`ExitCode`).

## Installer

`img-converter-setup.exe` is self-contained: `build.rs` embeds the worker and the DLL built beforehand for the same target and profile. `setup.rs` implements the app-independent flow (dialogs, file copy, Start Menu folder, *Settings → Apps* entry, update/repair/uninstall, architecture check) and `main.rs` describes img-converter through the `App` trait. Installing:

1. Copies `img-converter.exe`, `img_converter_shell.dll` and `app.ico` to `%LOCALAPPDATA%\Programs\img-converter` (a DLL locked by Explorer is renamed out of the way first), plus a copy of the installer named `img-converter-uninstall.exe`.
2. Creates `%LOCALAPPDATA%\img-converter\config.ini` if needed.
3. Registers the COM server and the context menu handlers under `HKCU`.
4. Creates the *img-converter* Start Menu folder (shortcut to `config.ini`) and the *Settings → Apps* entry.
5. Suggests the AV1 Video Extension or the Raw Image Extension when Windows cannot decode AVIF or current camera RAW files.

Uninstalling removes all of the above, including the settings folder.
