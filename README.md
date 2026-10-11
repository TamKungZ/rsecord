# rsecord

A lightweight, cross-platform screen recorder built with Rust and [iced](https://iced.rs). It records your screen and audio straight to MP4 with no external FFmpeg.

## Features

- 30 FPS screen capture of the primary display
- H.264 video (OpenH264) and AAC-LC audio (FDK-AAC), muxed into a single MP4
- Audio source: none, microphone, or system audio (Windows only)
- Live recording timer, status line, and saved file path
- Bitrate scales automatically with resolution (1 to 20 Mbps)

## Output

Files are saved to `Videos/rsecord/`, falling back to `~/rsecord/` if the Videos folder is unavailable.

Filename format: `rsecord-YYYY-MM-DD_HH-MM-SS.mp4`

## Requirements

- Rust 1.85 or newer (edition 2024)
- A C/C++ compiler and CMake (used to build OpenH264 and FDK-AAC)
- Linux only: development packages for ALSA, XCB, XRandR, D-Bus, PipeWire, Clang, and pkg-config

## Usage

1. Choose an audio source from the dropdown.
2. Click **Start Recording**.
3. Click **Stop** to finish. The file is written and its path is shown in the status line.

The audio source cannot be changed while recording.

## Platform notes

| Platform | Screen | Microphone | System audio                          |
|----------|--------|------------|---------------------------------------|
| Windows  | Yes    | Yes        | Yes                                   |
| Linux    | Yes    | Yes        | Yes, via a PulseAudio/PipeWire monitor |
| macOS    | Yes    | Yes        | Requires a virtual device (BlackHole) |

## License note

FDK-AAC carries licensing and patent considerations. Review the applicable terms before commercial distribution.