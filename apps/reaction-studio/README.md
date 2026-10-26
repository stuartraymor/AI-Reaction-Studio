# Reaction Studio workspace (L6)

The native app and `reaction-pipeline` reuse FilmCraft Session commands for import, speech, graphics, timeline edits, captions, export and project persistence. `ChatProvider` isolates the pinned local llama.cpp writer/vision service. Source fingerprints, evidence quotations and integer Tick timestamps are retained in saved analysis/scripts.

## Revision and captions (0.3)

`reaction-pipeline regenerate ANALYSIS INPUT_SCRIPT OUTPUT_SCRIPT ZERO_BASED_INDEX [DIRECTIONS]` generates one replacement draft. The selected source quote, timestamp and all other reactions are immutable. Invalid/repeated replies return an error. Success resets `review_required`; the GUI resets rendering approval and saves a backup of the previous script. This is model guidance, not a guarantee of factual correctness or perspective fidelity.

`reaction-pipeline render ANALYSIS SCRIPT OUTPUT --approve --subtitles` creates commentary captions through `captions.newTrack/add/setTimes`, exports an SRT, and burns captions into the MP4 through `file.exportMedia`. Captions stay editable in the project and are compared after project reopening. Subtitle timing is estimated by word counts over measured narration duration; it is not forced alignment or source-dialogue transcription. Cues stay within narration, use frame-aligned integer boundaries, and are positioned above the simple bottom-right character. Omit `--subtitles` to keep the previous uncaptained export behavior.

The GUI offers per-reaction regeneration directions/buttons and a persistent commentary-subtitle checkbox. Settings from 0.2 retain existing choices and receive the new default. Existing analysis/scripts remain schema-compatible. Saved production metadata records the caption option and timing method.

## Simple mode (0.4)

See [PLAN.md](PLAN.md). Modules:

- `source.rs` parses the single video box (file path or YouTube link; only the validated 11-character id is kept).
- `download.rs` runs the user's own `yt-dlp` (`REACTION_STUDIO_YTDLP` or PATH), never shipped or linked: `--ignore-config --no-playlist`, the canonical URL after `--`, 15 min / 2 GB / no-live limits, cancellable (whole process tree), resumable, MP4-header checked, reused on later runs.
- `style.rs` holds the six reaction presets, migration of pre-0.4 settings, `resolve` (what the writer gets) and `motion` (figure mood and strength).
- `avatar.rs` is the pure pose maths (bob, brows, blink, mouth) used both by `render.rs` keyframes and the GUI card previews.
- `pipeline.rs` has `draft` (download → analysis, reused when it matches the same unchanged file → script) and `make_reaction` behind `reaction-pipeline make`.

`Script.style` is new and optional (`#[serde(default)]`), so older scripts still load; `production.json` records `style` and `mood`.

## Verification

`cargo test -p filmcraft-reaction-studio` covers link parsing, downloader arguments/progress/errors/cancel/timeout, style presets and migration, bounded poses and keyframes, pipeline argument parsing and analysis reuse, drop handling, evidence validation, immutable revision behavior, rejected model replies, hostile indices/timing, caption text preservation and older-settings migration. Provider unit tests supply deterministic responses to test boundary behavior; actual local inference and MP4 decoding are tested separately and recorded in the repository development record. `scripts/verify-analysis-reaction.py` checks exported holds, source resume matching, narration alignment and reopening. It uses external FFmpeg only as a test oracle.

See [user/build instructions](../../REACTION-STUDIO.md), [model/runtime pins](models.json), and [development evidence](../../REACTION-STUDIO-DEVELOPMENT.md). No model weights or user media are committed.
