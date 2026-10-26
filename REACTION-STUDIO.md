# AI Reaction Studio 0.4 Preview

Local Windows workspace built on the open-source FilmCraft editing engine. Paste a YouTube link or drop in a video, pick how the animated figure reacts, and get an editable pause-and-react video. Transcription, frame descriptions, commentary and voices all run on this computer.

## Install

Run `AI-Reaction-Studio-0.4.0-Preview-Setup.exe` from this fork's GitHub Releases. Installation is per user, with a desktop shortcut and uninstaller. Requires 64-bit Windows and the Microsoft Visual C++ x64 runtime already installed. This development preview is unsigned. The installer includes the local llama.cpp runtime and an unmodified FilmCraft editor, with license notices; no paid service or API key is required.

On first launch the **Getting started** panel shows what is missing. Select **Download local models** and allow approximately 3.6 GB for verified model downloads. Models, settings, downloads and projects live under `%LOCALAPPDATA%\AI Reaction Studio`; uninstall preserves them. Allow several minutes for CPU inference and at least 8 GB available RAM; the writer alone used approximately 5 GB in testing.

**YouTube links (optional).** Install [yt-dlp](https://github.com/yt-dlp/yt-dlp#installation) yourself (for example `winget install yt-dlp.yt-dlp`), or set `REACTION_STUDIO_YTDLP` to its path. AI Reaction Studio runs it as a separate program; it is not shipped, bundled or linked. Only download videos you own or have permission to use, and follow YouTube's Terms of Service. Without yt-dlp, download the video another way and drop the file in.

## Using it

1. **Video:** paste a YouTube link or a file path, use **Browse…**, or drag a video onto the window. The line under the box says what was recognised.
2. **How should it react?** Pick **Hype, Funny, Skeptic, Debate, Explainer** or **Chill**; each card's figure shows the moves you'll get. Add an optional **Anything else?** line. **Advanced** holds the perspective, intensity, instructions, voice, figure, reaction count, subtitles and project folder (changing these makes the style "Custom").
3. **Make my reaction:** downloads (for links), analyzes and drafts the commentary. Check **Here's what it will say**, edit or regenerate any line, tick OK and choose **Make the video**.

The result offers **Watch**, **Save MP4…**, **Try another style** (reuses the download and analysis), the project folder and the FilmCraft editor. Each render folder holds the MP4, editable `.fcproj`, narration WAVs, analysis, approved script and timeline provenance; `production.json` records the style and figure mood.

## What this preview supports

* 3-second to 15-minute videos from local files or single YouTube videos (H.264/AAC MP4 at 720p or less, up to 2 GB; no playlists or live streams). Downloads can be cancelled and resume next time.
* Whisper tiny transcription, candidate cut detection, SmolVLM2-500M visual observations and Qwen3-4B-Instruct-2507 commentary, all on CPU.
* Six reaction styles plus Custom; three Kokoro voices and two original procedural characters.
* A figure with eyes, blinking, eyebrows and body movement that follow the style's mood (excited bounce, raised skeptical brow, amused squint, fierce frown, calm nod), exaggerated for a short beat when the video pauses, plus loudness-driven mouth movement.
* Pause-and-react editing at 640x360/24 fps, optional commentary subtitles, separate source/narration tracks and project reopen verification.

Model descriptions and argument interpretations can be wrong. Sampled frames do not cover every event. A style is guidance, not a guarantee of viewpoint fidelity. Every generated reaction is labelled opinion and requires review; no independent factual verification occurs. Speaker identity is not inferred. Phoneme lip sync, more characters, other editing layouts and broad production QA remain future work.

## Build and package

Requires Rust 1.95+, MSVC Build Tools and Windows SDK. Run `cargo build --locked -p filmcraft-reaction-studio --bins`. Place a verified llama.cpp b11744 Windows CPU runtime in `runtime/` beside the executables, or set `REACTION_LLAMA_SERVER` to its server executable. `REACTION_STUDIO_DATA_DIR` overrides app data for isolated testing.

`reaction-pipeline` exposes `setup`, `make LINK_OR_FILE --out DIR [--style ID] [--extra TEXT] [--count N] [--approve]`, `download LINK_OR_FILE`, `analyze LINK_OR_FILE OUT`, `script ANALYSIS OUT PERSPECTIVE INTENSITY COUNT`, `regenerate …` and `render ANALYSIS SCRIPT OUT --approve`. Without `--approve`, `make` stops at the draft so it can be reviewed. Render refuses to overwrite an existing saved project. The native workspace calls the same pipeline and FilmCraft Session commands.

For packaging, generate dependency metadata with `cargo metadata --locked --filter-platform x86_64-pc-windows-msvc --format-version 1 > target/reaction-metadata.json`, then run `python scripts/collect-reaction-licenses.py target/reaction-metadata.json target/reaction-third-party-licenses.txt`. Run `packaging/windows/package-reaction.ps1` with `-NsisDirectory`, `-RuntimeDirectory`, `-EditorDirectory` (unmodified FilmCraft binaries) and `-OutputDirectory`. Model pins are in `apps/reaction-studio/models.json`; full notices ship with the installer. Models, yt-dlp and test media are excluded from Git and from the installer.

The earlier deterministic prototype remains available through `scripts/reaction-prototype.ps1`. See [development record](REACTION-STUDIO-DEVELOPMENT.md) for actual acceptance evidence and limitations, and [the simple-mode plan](apps/reaction-studio/PLAN.md) for what is done and next.
