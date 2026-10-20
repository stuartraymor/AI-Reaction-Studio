# AI Reaction Studio — development record

## Foundation
- Official upstream: https://github.com/storytold/filmcraft
- Pinned baseline: 105c09bd80e2c86ec7464a2c59a2501eb89ffc5c (0.9.0 workspace).
- Local branch: ai-reaction-studio. No remote fork or publication created.
- License: MIT OR Apache-2.0; retain upstream notices. ArtCraft trademarks are separately restricted and must be removed from a branded release.
- Read AGENTS.md, CLAUDE.md, README.md, architecture, contributing, testing, agents, roadmap and gaps before implementation.
- Windows: MSVC 18 BuildTools; AMD Radeon 780M + NVIDIA RTX 4060 Laptop GPU.
- Rust 1.95 installed into the task's work/toolchain; system PATH unchanged.

## Architecture decision (provisional until runtime acceptance)
Keep FilmCraft's Rust engine, timeline, codecs, audio mixer, atomic project persistence and egui editor. Dispatch existing Session commands through CLI/MCP. Add deterministic production jobs and a Reaction Studio workspace above the engine. Preserve media and commentary as separate editable tracks in .fcproj files. Use integer ticks for all timeline planning.

The first acceptance fixture is original procedural footage, a reviewed fixed script, built-in local formant TTS and a simple shape character with animated mouth. This proves editing, not AI video understanding. No cloud uploads, billable API calls or claims of machine analysis.

## Capability evidence
Source includes CLI run/exec, stdio MCP, desktop loopback bridge, timeline.place, sequence.addTracks, graphics layers/keyframes, tts.create, file.exportMedia, jobs.list/cancel and file.save. Presence in source is not a runtime pass.
Upstream describes itself as alpha. Windows launch/audio, codec coverage, hardware acceleration and AI model configuration need acceptance tests. Whisper and neural voices are optional features; default built-in voices need no downloaded model. Upstream documents missing AI features and gaps; no production suitability conclusion yet.

## Milestones
- [x] Identify official repository, inspect license and instructions, clone separate tree and branch.
- [x] Locate MSVC and GPUs; install isolated Rust.
- [x] Build unchanged desktop and CLI.
- [x] Launch unchanged editor with isolated data directory.
- [x] Import fixture, edit timeline/tracks, mix audio and export MP4.
- [x] Implement first repeatable reaction pipeline, save editable project, render and inspect MP4.
- [x] Add transcription/scene detection/multimodal analysis with timestamp provenance.
- [x] Add reviewed local script generation, perspectives and a replaceable chat-provider interface.
- [ ] Add improved avatars, voices and speech-driven lip synchronization.
- [ ] Add deterministic orchestration, retry/cancel/recovery and MCP integration tests.
- [x] Add dedicated workspace with saved analysis and editable scripts.
- [ ] Add individual segment regeneration.
- [x] Package and test a Windows preview installer, uninstaller and shortcuts.
- [ ] Complete broad Windows workflow QA and all editing modes.

## Release gates
Do not label the full application complete until actual Windows install/uninstall, reopening saved projects, multiple source clips, all editing modes, subtitle/audio sync, provider errors and cancellation have been verified. Run upstream required quality gates before committing. No commits until green gates.

## Development log — 2026-10-19
Repository cloned and both baseline binaries built unchanged in 7m55s. Official 0.9.0 portable release was also downloaded and checksum-verified to accelerate independent validation. The source-built editor passed import, sequence creation, adding tracks, audio-level editing, saving/reopening and H.264/AAC export. The actual stdio MCP server passed handshake, tool listing, project/sequence creation and inspection.

The first Rust reaction prototype built with the existing neural-voice feature and generated an 11.5-second, 640x360, 24 fps H.264/AAC MP4 from six seconds of original procedural footage. Kokoro Heart produced 5.47 seconds of local speech; the source is held for 5.5 seconds and then resumes. The original audio has a matching gap, the narration begins at exactly two seconds, and the shape character has 66 audio-driven mouth keyframes. The project contains three editable video tracks and two audio tracks. Saving/reopening preserved the track contents, keyframes and duration exactly, with no missing media. Repeating the job produced a byte-identical MP4.

Full external audio/video decoding passed. Visual contact-sheet inspection passed; independent pixel checks measured less than 0.201/255 mean channel change during the hold and confirmed resumed motion. Voice samples are finite with nonzero RMS. Existing-project overwrite and oversized-script inputs were refused. The new example passed Clippy with warnings denied. Formatting passed per package; the single workspace-wide invocation exceeded Windows command-line length, so the equivalent package-by-package check was used.

Windows playback is not fully accepted: default graphics displayed the source and played 139/144 frames while compiling, but the audio device provided zero callbacks and the app reported stalled output. Forced software graphics could not create a surface on this machine. These remain foundation/platform QA issues; output video decoding and audio data checks do not prove desktop loudspeaker playback.

The full upstream workspace test/lint/wasm release gate has not been run. This is a user-requested public development checkpoint, not a production release or a claim that all upstream quality gates pass. No billable calls or video uploads were made. Model/service choices remain subject to license and privacy checks.


## 0.2 analysis milestone — 2026-10-19

Added a native Reaction Studio workspace and CLI on the existing FilmCraft Session engine. Actual local Whisper transcription, candidate scene cuts and SmolVLM2 frame descriptions feed schema-constrained Qwen3-4B commentary. Scripts retain exact source quotations, timestamps and source fingerprints. Generated claims are tentative and unverified; manual approval is required before narration/rendering. Eight perspectives, intensity, custom directions, editable drafts, three Kokoro voices and two original amplitude-animated characters are available. This remains a development preview, not the complete master specification.

Reused FilmCraft's imports, Whisper implementation/download catalog, TTS, graphics/keyframes, timeline edits, H.264/AAC exporter and project persistence. Reused pinned MIT llama.cpp b11744 for local VLM/writer inference; Apache-2.0 model pins and runtime archive SHA256 are recorded in `apps/reaction-studio/models.json`. NSIS 3.11 creates a per-user installer with desktop/start-menu shortcuts and a specific-file uninstaller that retains data/models/projects. Full Rust/runtime license texts are packaged. The unchanged baseline FilmCraft editor ships in a separate editor directory.

Actual acceptance source: 13.916-second original procedural video narrated locally with Kokoro. Whisper found three timestamped evidence passages. Five sampled frames were described by the local VLM; it incorrectly called the sun a moon, demonstrating the need for review. Single-speaker clustering was disabled after an initial false split. The first 1.5B writer repeated commentary and handled perspectives weakly; replaced with Qwen3-4B. Educational and Opposing drafts were generated with real inference. The Opposing draft did not reliably oppose the source, a documented model limitation; no viewpoint accuracy claim is made.

A reviewed two-reaction Educational draft produced a 28.1667-second MP4 and editable project. External FFmpeg decoded the entire audio/video stream successfully. Both holds had stable source backgrounds (maximum mean RGB difference 0.255), separate narration aligned within the hold, and no source-audio overlap. Both voice WAVs contained finite samples with RMS above 0.045. Project reopening preserved video/audio tracks, duration and frames exactly with no missing media. Visual contact-sheet inspection confirmed original character geometry, mouth movement and readable opinion labels. The native workspace was launched and its own screenshot inspected. A CLI relative-output-path reopening failure was reproduced and corrected by canonicalizing output folders before asset creation.

Limits: CPU writer calls can take about two minutes; up to 15-minute sources, eight frames, four reactions, English commentary, one pause-and-react layout at 640x360/24 fps. Cancellation waits for the current bounded model stage. No YouTube import, independent web fact verification, subtitles or advanced avatars. Original editor preview audio remains a known development-machine issue; exported audio decodes correctly.


Windows packaging acceptance: the 101,298,604-byte installer completed silently with exit 0 into a path containing spaces. The installed native app launched and loaded a saved analysis; model setup verified all current files without redownloading. The installed CLI then analyzed the actual source with the bundled runtime: three evidence passages, five visual observations and frame-grid timestamps. Uninstall returned 0, removed packaged executables, desktop shortcut and its registration, and retained both model/project data and an unlisted file placed in the install folder. The app was then installed in the normal per-user Programs directory and launched with its desktop shortcut available. Installer SHA256: `ca148d844fe57be89a67a480c32faaa949d04474a625aa788908fe305ee36c0c`.

Focused checks: 13 Reaction Studio tests and 16 speech unit tests passed, plus the speech model integration target (its optional external-fixture case skips internally without that fixture). Actual model acceptance above is separate evidence. Package-by-package formatting passed across the workspace (avoids Windows command-length limits); layering and asset attribution checks passed. Full workspace Clippy initially found an inherited Whisper `expect()` and five UI/audio lints: replaced the table conversion with fallible propagation and a complete/bijective-table regression test, and simplified the UI/audio conditions equivalently. Full workspace test compilation was stopped before exhausting C: disk; no full-workspace test pass is claimed. Built binaries/license inventories were preserved, and Cargo's own clean command reclaimed the build cache. The user's explicit instruction to keep saving progress to GitHub authorizes this documented preview checkpoint despite the incomplete upstream full-test gate; it is not a production-ready release.

Final verification: workspace/all-target Clippy passed with warnings denied after the equivalent lint fixes; WebAssembly checks passed for all 49 selected crates; layering passed; attribution passed for all 71 assets. Formatting passed package by package. Full-workspace execution tests remain incomplete for the disk-space reason above. Installer binaries were built before cache cleanup and match the tested installed application.
