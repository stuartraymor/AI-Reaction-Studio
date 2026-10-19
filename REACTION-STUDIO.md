# AI Reaction Studio

Development fork of [FilmCraft](https://github.com/storytold/filmcraft). The immediate milestone is a genuine local reaction-video pipeline. The complete one-click AI application, dedicated workspace and installer are not implemented yet.

## Local acceptance prototype

Requires Windows 11, MSVC Build Tools and Rust 1.95 or later. No API keys, paid services or video uploads. The optional Kokoro model download is approximately 336 MB. Its weights/voices are declared Apache-2.0 on the [official model card](https://huggingface.co/hexgrad/Kokoro-82M); CMUdict is BSD-style. Model and generated media files stay outside Git.

```powershell
git clone https://github.com/stuartraymor/AI-Reaction-Studio.git
cd AI-Reaction-Studio
git switch ai-reaction-studio
./scripts/reaction-prototype.ps1
# Or supply a 3–60 second authorized local video and a reviewed UTF-8 script:
./scripts/reaction-prototype.ps1 -Source C:/Videos/test.mp4 -Script C:/Videos/script.txt
```

The script builds the existing CLI and the Rust prototype, downloads the pinned local voices through FilmCraft's existing downloader, and uses original procedural footage if no source is supplied. The reaction pauses the source at two seconds, speaks a reviewed script with a local neural voice, shows a simple original shape character with audio-energy-driven mouth animation, and resumes the original video. Output is 640×360 at 24 fps. The prototype has deliberate short-video limits; it does not analyze the video or generate factual commentary.

Output includes `reaction.mp4`, editable `reaction.fcproj`, separate narration WAV, reviewed script, timestamp provenance, timeline inspection, export result and reopened timeline. Reruns refuse to overwrite a saved project. Original audio is absent during the inserted pause; the source audio resumes with its matching picture. Mouth motion is basic amplitude animation, not phoneme-accurate lip sync. No subtitles or advanced avatar provider are implemented in this prototype.

Open the editable project:

```powershell
cargo run -p filmcraft -- C:/path/to/reaction.fcproj
```

FilmCraft's stdio MCP server remains available through `filmcraft-cli mcp`; both MCP and this deterministic pipeline use the same `Session::execute` commands. The prototype adds no second editing or rendering engine.

See [development record](REACTION-STUDIO-DEVELOPMENT.md) for milestone status, actual acceptance evidence and remaining work. Upstream licenses and asset notices remain in place. ArtCraft branding has separate trademark restrictions and must be replaced before distributing a branded application release.

## Repeat acceptance checks

```powershell
python -m pip install --target target/qa imageio-ffmpeg pillow
$env:PYTHONPATH = "$PWD/target/qa"
python scripts/verify-reaction-video.py C:/path/to/default-fixture-output
python scripts/test-reaction-mcp.py
```

The video checker targets the moving default fixture and fixed prototype layout. It decodes the complete output with external FFmpeg, checks freeze/resume frames, narration timing, source-audio gaps, mouth keyframes, finite voice samples and project reopening, and produces a contact sheet. It is test tooling, not a runtime video-processing dependency. Static custom footage needs different motion acceptance criteria.
