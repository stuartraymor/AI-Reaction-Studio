# AI Reaction Studio

A local Windows reaction-video workspace built on the open-source FilmCraft engine.

**[Download the Windows 0.2 preview installer](https://github.com/stuartraymor/AI-Reaction-Studio/releases/tag/reaction-v0.2.0-preview.1)**

Choose a video, transcribe and analyze sampled frames locally, generate timestamp-linked commentary, review the script, and render a reaction video with an editable timeline. No API key or paid AI service is required.

The installer includes a desktop shortcut, uninstaller, local llama.cpp runtime and unmodified FilmCraft editor. Models download separately (about 3.6 GB). Requires Windows x64, the Microsoft Visual C++ x64 runtime and adequate memory for local inference.

This is a development preview. Model observations may be wrong; all commentary requires review and is not independently fact checked. The current export style is Pause and React at 640x360/24 fps with simple procedural characters. YouTube import, subtitles, advanced avatars and additional editing styles remain future work. The full upstream test suite has not completed on the development machine; see the development record for precise verification.

- [Install, use, build and package](REACTION-STUDIO.md)
- [Development status and acceptance evidence](REACTION-STUDIO-DEVELOPMENT.md)
- [Model/runtime pins](apps/reaction-studio/models.json)
- [Third-party notices](packaging/windows/REACTION-THIRD-PARTY.txt)
- [Original FilmCraft documentation](README-FILMCRAFT.md)

Source is MIT OR Apache-2.0 with upstream and asset notices preserved. This fork reuses FilmCraft, Whisper, Kokoro, SmolVLM2, Qwen3 and llama.cpp; model licenses and download checksums are documented separately.
