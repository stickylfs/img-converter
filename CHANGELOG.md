# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/).

## [1.0.0] - 2026-10-13

First public release.

### Added
- **Convert** entry in the File Explorer context menu for images and PDFs, converting the selected files to JPEG, PNG, WebP, AVIF, GIF, BMP, TIFF, ICO, TGA, HDR, OpenEXR or PDF, fully offline.
- Input formats include HEIC/HEIF, AVIF and camera RAW (through the Windows codecs), SVG, DDS, HDR and OpenEXR.
- Several images converted to PDF are merged into one document in natural file-name order; PDF pages can be converted to images.
- EXIF orientation is applied, and ICC color profiles and EXIF metadata are kept for JPEG, PNG and WebP output (`preserve_metadata`).
- Atomic writes and collision-safe output names: an existing file gets a numbered name (`photo (1).png`) instead of being overwritten, unless configured otherwise.
- Error dialog that explains each failed file in plain language, shows the technical details on demand and offers **Open log** and, for a missing AVIF/HEIC/RAW codec, its Microsoft Store page.
- `config.ini` for formats, quality, scale, transparency, metadata and limits, opened from **Convert → Configure...** or the Start Menu.
- `img-converter.exe` command line for scripts, with list files for very large selections and meaningful exit codes.
- English and Spanish.
- Per-user installer (`img-converter-setup.exe`) for x64, x86 and ARM64: no administrator rights, update/repair, unattended mode, clean uninstall, and a suggestion to install the AV1 Video Extension or the Raw Image Extension when Windows lacks them.
