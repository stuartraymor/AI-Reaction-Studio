# AI Reaction Studio 0.2 Preview

Local Windows workspace built on the open-source FilmCraft editing engine. This preview adds real video transcription, candidate scene cuts, sampled-frame descriptions, timestamp-linked script generation, local voices and editable pause-and-react videos.

## Install

Run `AI-Reaction-Studio-0.2.0-Preview-Setup.exe` from this fork's GitHub Releases. Installation is per user, with a desktop shortcut and uninstaller. Requires 64-bit Windows and the Microsoft Visual C++ x64 runtime already installed. This development preview is unsigned. The installer includes the local llama.cpp runtime and an unmodified FilmCraft editor, with license notices; no paid service or API key is required.

Launch AI Reaction Studio, select **Download local models**, and allow approximately 3.6 GB for verified model downloads. Models, settings and projects live under `%LOCALAPPDATA%\AI Reaction Studio`; uninstall preserves them. Inference stays on this computer. Internet is used to download the pinned models, not to upload source media. Allow several minutes for CPU inference and at least 8 GB available RAM; the writer alone used approximately 5 GB in testing.

Choose a local video, analyze it, choose perspective/intensity and generate a script. Review and edit each reaction, approve it, then render. The output includes an MP4, editable `.fcproj`, separate narration WAVs, analysis, approved script and timeline provenance. Use **Open in editor** for further edits. Keep the source video and generated assets available when reopening projects.

## What this preview supports

* Local 3-second to 15-minute videos, up to eight sampled frames and one to four reactions.
* Whisper tiny transcription, candidate cut detection, SmolVLM2-500M visual observations and Qwen3-4B-Instruct-2507 commentary, all on CPU.
* Eight perspective choices, intensity and custom directions; three Kokoro voices and two original procedural characters.
* Pause-and-react editing at 640x360/24 fps, separate source/narration tracks, source-audio gaps during holds, basic audio-energy mouth animation and project reopen verification.
* Editable saved drafts, progress reporting, errors, and cancellation at stage boundaries (an active model request may take up to three minutes to return).

Model descriptions and argument interpretations can be wrong. Sampled frames do not cover every event. Perspective selection is guidance, not a guarantee of viewpoint fidelity. Every generated reaction is labelled opinion and requires review; no independent factual verification occurs. Speaker identity is not inferred. YouTube import, subtitles, advanced avatars, phoneme lip sync, all requested editing styles and broad production QA remain future work. The bundled original editor has a known preview-audio issue on the development machine; exported audio passed decoding.

## Build and package

Requires Rust 1.95+, MSVC Build Tools and Windows SDK. Run `cargo build --locked -p filmcraft-reaction-studio --bins`. Place a verified llama.cpp b11744 Windows CPU runtime in `runtime/` beside the executables, or set `REACTION_LLAMA_SERVER` to its server executable. `REACTION_STUDIO_DATA_DIR` overrides app data for isolated testing.

`reaction-pipeline` exposes `setup`, `analyze SOURCE OUT`, `script ANALYSIS OUT PERSPECTIVE INTENSITY COUNT`, and `render ANALYSIS SCRIPT OUT --approve`. Render refuses to overwrite an existing saved project. The native workspace calls the same pipeline and FilmCraft Session commands.

For packaging, generate dependency metadata with `cargo metadata --locked --filter-platform x86_64-pc-windows-msvc --format-version 1 > target/reaction-metadata.json`, then run `python scripts/collect-reaction-licenses.py target/reaction-metadata.json target/reaction-third-party-licenses.txt`. Run `packaging/windows/package-reaction.ps1` with `-NsisDirectory`, `-RuntimeDirectory`, `-EditorDirectory` (unmodified FilmCraft binaries) and `-OutputDirectory`. Model pins are in `apps/reaction-studio/models.json`; full notices ship with the installer. Models and test media are excluded from Git.

The earlier deterministic prototype remains available through `scripts/reaction-prototype.ps1`. See [development record](REACTION-STUDIO-DEVELOPMENT.md) for actual acceptance evidence and limitations.
