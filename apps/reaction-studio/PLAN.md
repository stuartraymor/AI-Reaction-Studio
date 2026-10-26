# AI Reaction Studio — "simple mode" plan

Goal: one window, three steps. **Paste a YouTube link (or pick a downloaded video) → pick how the figure reacts → get a reaction video.** Everything that exists today (analysis, reviewed scripts, voices, captions, editable project) stays, but moves behind an **Advanced** expander.

## What the user sees

| Step | Screen | What happens |
|---|---|---|
| 1. Video | One text box: *"Paste a YouTube link or drop a video file"* + **Browse…** + drag-and-drop | `source::parse_source` decides link vs. file; the line under the box says what it found. Links are downloaded to `Data/downloads/<id>.mp4`, then analyzed exactly like a local file. |
| 2. Reaction | Six cards: **Hype, Funny, Skeptic, Debate, Explainer, Chill**, each with a live animated preview of the figure | `style::STYLES` turns one pick into perspective + intensity + writer instructions + figure mood. Optional one-line *"Anything else?"* box. |
| 3. Make it | **Make my reaction**, then *"Here's what it will say"* (edit / regenerate each line, tick OK), **Make the video**, then **Watch** / **Save MP4…** / **Try another style** / editor | `pipeline::draft` → review → `render`. "Try another style" reuses the download and analysis. |

Defaults: Hype style, 3 reactions, captions on. Voice and figure colour are the user's own choice under Advanced; they do not change with the style.

## Milestones

### S1 — One input box (link or file) ✅
- [x] `parse_source`: accepts youtube.com/watch, youtu.be, shorts, embed, live, m./music. hosts; rejects lookalike hosts, credentials in URL, playlists, non-http schemes, control characters, oversized input. Rebuilds a canonical URL from the 11-char id only.
- [x] `download.rs`: runs the user's own `yt-dlp` (`REACTION_STUDIO_YTDLP` or PATH; never shipped or linked). Only the validated id goes on its command line, after `--`, with `--ignore-config --no-playlist`. Picks one H.264/AAC MP4 of 720p or less, so no ffmpeg merge is needed. Caps: 15 min, 2 GB, no live streams, 1 h timeout. Cancel stops the whole process tree; partial data in `<id>.partial` resumes next time; a finished file is used only after an MP4 header check, and is reused on later runs. yt-dlp output becomes one plain sentence.
- [x] CLI: `reaction-pipeline download LINK_OR_FILE`; `analyze` accepts a link directly.
- [x] GUI: single box + Browse + drag-and-drop (a video becomes the source, a dropped `analysis.json` opens it, anything else gets a clear message); rights notice shown under the box whenever a link is entered.
- [ ] Try against real YouTube with yt-dlp installed (yt-dlp is not on the dev PC yet).

### S2 — "How should it react?" ✅
- [x] Six presets, each valid for the writer (`analysis::PERSPECTIVES`), with a distinct `Mood` and a bounded `motion_gain`.
- [x] Settings store a `style` id; pre-0.4 settings migrate to the preset with the same perspective and nearest intensity, otherwise "custom"; unknown ids fall back to `hype`.
- [x] GUI cards with live previews; perspective/intensity/instructions/voice/figure/count/subtitles/project folder moved under **Advanced**. Editing perspective, intensity or instructions there switches the style to "Custom".
- [x] `Script.style` (optional field, older scripts still load) records the choice; the review list labels each line with it.

### S3 — A figure that actually reacts ✅
- [x] Brows, blinking eyes and body bob as separate graphics layers on the figure (`render.rs`), keyframed from `avatar::pose`.
- [x] Mood recipes: Excited = bounce + raised brows; Skeptical = one raised brow; Amused = squint + quick shake; Fierce = lowered, angled brows + lean; Calm = slow nod. Scaled by `motion_gain`. Louder moods also open the mouth wider.
- [x] Reaction beat: the pose is exaggerated for the first 0.3 s of each pause. (Narration still starts at the pause, so timing, captions and the verification script are unchanged.)
- [ ] Whole-head tilt: graphic layers rotate about their own centre, so a true tilt needs a group transform. Later.
- [ ] More characters (still original, procedural, attribution-clean). Later.

### S4 — One-button pipeline ✅
- [x] `pipeline::draft(source, folder, style, writer, count)` chains download → analysis (reused when the folder's `analysis.json` belongs to the same unchanged file) → script; `make_reaction` adds the render when approved.
- [x] CLI: `reaction-pipeline make LINK_OR_FILE --out DIR [--style funny] [--extra TEXT] [--count N] [--voice ID] [--character Teal] [--no-subtitles] [--approve]`. Without `--approve` it stops at the draft for review.
- [x] Result screen: Watch, Save MP4…, Try another style (keeps the analysis), project folder, editor. `production.json` records `style` and `mood`.

### S5 — Polish & release ✅ (except real-world checks)
- [x] First-run **Getting started** panel: model check (by size, instant) with the download button, yt-dlp check with an install link, **Check again**. Hidden once everything is ready.
- [x] Version 0.4.0 in the crate, window, installer script and NSI; REACTION-STUDIO.md and the crate README rewritten for the new flow.
- [ ] Build and try the installer; publish a release (needs your go-ahead).
- [ ] Screenshots for the docs.

## Tests

`cargo test -p filmcraft-reaction-studio`: **67 tests, all passing** (60 library + 7 app).

| Where | What it proves | Tests |
|---|---|---|
| `source.rs` | Common link shapes parse; hostile/lookalike/playlist/odd input rejected without panics; canonical URL holds only the id | 6 |
| `download.rs` | Safe arguments (option-smuggling ids rejected); progress parsing; plain error sentences; only real MP4s finalized; local files and finished downloads skip yt-dlp; PATH search; progress + errors from a stand-in process; cancel and timeout stop a hung process within seconds | 11 |
| `style.rs` | Unique ids; every preset accepted by the writer; styles differ; unknown ids fall back; bounded gain; settings migration; `resolve` (preset + extra line, Custom, length caps); `motion` from style or perspective | 9 |
| `avatar.rs` | Poses finite and bounded for any mood/gain/time (incl. NaN/∞/negative); zero gain stays still; each mood has its own face; reaction beat; blink rhythm; bounded mouth | 6 |
| `render.rs` | Face keyframes cover every layer and stay on screen for all moods; mouth-only frames; eyes never collapse | 3 |
| `pipeline.rs` | `make` argument parsing and every mistake rejected; analysis reused only for the same unchanged file (not another file, an edited file or a corrupt report); bad style/source/missing file fail before any stage starts | 4 |
| `providers.rs` | First-run model check: empty or wrong-sized folders reported missing, correct size accepted | 1 |
| `lib.rs` | Older scripts without `style` still load; oversized style id rejected; plus the existing evidence/approval tests | 2 new |
| `main.rs` | Drop routing; source hint; first-run default style; pre-0.4 settings migration and junk values; card → Advanced values; script/settings matching | 6 new |
| existing | Evidence-grounded scripts, immutable revisions, captions | — |

Not covered yet:

| What | Why / next step |
|---|---|
| End-to-end `make` with a real video | `analysis::generate` starts the local model directly; it needs a `ChatProvider` parameter so a test can use a fake writer with an ffmpeg fixture in `target/fixtures/`. |
| Real yt-dlp downloads | Manual: public video, private video, >15 min video, cancel then resume. |
| Rendered figure motion | Extend `scripts/verify-analysis-reaction.py` to check the new figure layers and keyframes per mood. |
| GUI clicks | Startup screen checked by screenshot; click-through of Make → review → render needs the models installed. |
| Clippy | Not installed in the toolchain used here; run `cargo clippy -p filmcraft-reaction-studio --all-targets -- -D warnings`. |

## Decisions

1. **Downloading:** external `yt-dlp` the user installs; if missing, the app explains how, or to choose a downloaded file. *(decided)*
2. **Rights:** the notice is shown under the box every time a link is entered. *(done)*
3. **Review step:** still required before rendering in the app. The CLI has `--approve` for unattended runs. An "auto-approve" switch in the app is your call.
