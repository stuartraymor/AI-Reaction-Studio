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
- [ ] Add transcription/scene detection/multimodal analysis with timestamp provenance.
- [ ] Add reviewed script generation, perspectives and provider configuration.
- [ ] Add improved avatars, voices and speech-driven lip synchronization.
- [ ] Add deterministic orchestration, retry/cancel/recovery and MCP integration tests.
- [ ] Add dedicated workspace and segment regeneration.
- [ ] Complete Windows workflow QA and packaging/installer/uninstaller/shortcut tests.

## Release gates
Do not label the full application complete until actual Windows install/uninstall, reopening saved projects, multiple source clips, all editing modes, subtitle/audio sync, provider errors and cancellation have been verified. Run upstream required quality gates before committing. No commits until green gates.

## Development log — 2026-10-19
Repository cloned and both baseline binaries built unchanged in 7m55s. Official 0.9.0 portable release was also downloaded and checksum-verified to accelerate independent validation. The source-built editor passed import, sequence creation, adding tracks, audio-level editing, saving/reopening and H.264/AAC export. The actual stdio MCP server passed handshake, tool listing, project/sequence creation and inspection.

The first Rust reaction prototype built with the existing neural-voice feature and generated an 11.5-second, 640x360, 24 fps H.264/AAC MP4 from six seconds of original procedural footage. Kokoro Heart produced 5.47 seconds of local speech; the source is held for 5.5 seconds and then resumes. The original audio has a matching gap, the narration begins at exactly two seconds, and the shape character has 66 audio-driven mouth keyframes. The project contains three editable video tracks and two audio tracks. Saving/reopening preserved the track contents, keyframes and duration exactly, with no missing media. Repeating the job produced a byte-identical MP4.

Full external audio/video decoding passed. Visual contact-sheet inspection passed; independent pixel checks measured less than 0.201/255 mean channel change during the hold and confirmed resumed motion. Voice samples are finite with nonzero RMS. Existing-project overwrite and oversized-script inputs were refused. The new example passed Clippy with warnings denied. Formatting passed per package; the single workspace-wide invocation exceeded Windows command-line length, so the equivalent package-by-package check was used.

Windows playback is not fully accepted: default graphics displayed the source and played 139/144 frames while compiling, but the audio device provided zero callbacks and the app reported stalled output. Forced software graphics could not create a surface on this machine. These remain foundation/platform QA issues; output video decoding and audio data checks do not prove desktop loudspeaker playback.

The full upstream workspace test/lint/wasm release gate has not been run. This is a user-requested public development checkpoint, not a production release or a claim that all upstream quality gates pass. No billable calls or video uploads were made. Model/service choices remain subject to license and privacy checks.
