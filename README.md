# Hyperjam

![Created with AI assistance](https://img.shields.io/badge/Created_with-AI_assistance-8A2BE2)

A native Rust seven-key rhythm gameplay player, inspired by O2Mania. This is a focused local player: load a chart, play its keysounds, and finish a session. It includes an original synthesized practice song, so no game assets are needed to try it.

This project was created with AI assistance from OpenAI Codex.

## Run

```sh
cargo run --release
cargo run --release -- /path/to/song.ojn
cargo run --release -- /path/to/song.ojn --difficulty hard --samples /path/to/song.ojm
```

Keep the sample bank named in the OJN header beside the chart; filenames are matched without case sensitivity. You can also drag an OJN and its OJM/OMC/M30 bank into the window together, or press **O** and paste an OJN path. A missing sample bank produces a warning and allows silent play. Sample decode failures are reported in the terminal.

Charts in `./music` are indexed automatically, including subfolders. Launching without a chart opens the music picker. Type a title, artist, or filename to search, use **Up/Down**, the mouse wheel, or page buttons to browse, then press **Enter** or click **Load chart**. **F1** reopens the picker and **F5** rescans the folder. The **Practice** button loads the built-in song. Use `--music /another/folder` to choose a different collection. Indexing reads only chart headers; audio is decoded when you load a chart.

On Linux, building needs `pkg-config`, ALSA development headers, and an X11/OpenGL runtime. For Debian/Ubuntu:

```sh
sudo apt install pkg-config libasound2-dev libx11-6 libxi6 libgl1-mesa-dev
```

The application uses Macroquad for rendering and Rodio for audio. Dependencies and versions are pinned in `Cargo.lock`.

## Controls

| Key | Action |
| --- | --- |
| **S D F Space J K L** | Seven note lanes, left to right |
| **Enter** | Start, resume, or play again |
| **Esc** | Pause / resume |
| **R** | Restart with a two-second countdown |
| **O** | Open a chart by path |
| **F1** | Music picker |
| **F2** | Configure lane keys |
| **F3 / F4** | Open a chart / restart, regardless of lane bindings |
| **Tab** | Toggle autoplay before starting |
| **Up / Down** | Scroll speed, 0.5–3.0x |
| **+ / -** | Timing offset, 10 ms steps |
| **F11** | Fullscreen |

Difficulty, autoplay, scroll speed, and volume can also be changed with the mouse. Positive timing offset delays the note highway and judgment clock relative to the audio; use it to compensate for output latency. The displayed offset is in milliseconds.

Press **F2** or click **Keys** to remap each lane. Click a lane and press its new key; assigning a key already used by another lane swaps the two lanes. **Save keys** writes the bindings to `hyperjam.keys` in the current directory, and the next launch restores them. **Defaults** restores S D F Space J K L in the editor; **Cancel** discards changes. Playback pauses while configuring keys.

Letters, numbers, modifiers, arrow-left/right, punctuation, and number-pad digits are supported. Menu controls such as Enter, Escape, Tab, function keys, and Up/Down stay reserved. R and O can be assigned to lanes; their retry/open shortcuts are then disabled. F3/F4 remain available. You can also override bindings for one session:

```sh
cargo run --release -- --keys "Z X C SPACE B N M"
```

Use `--bindings /path/to/profile.keys` for a separate saved profile. The file contains seven space-separated names, for example `S D F SPACE J K L`.

## Gameplay and formats

- OJN easy, normal, and hard charts; seven lanes, backing events, BPM changes, fractional measure lengths, per-event gain/pan, and long notes.
- OJM PCM/OGG banks, encrypted OMC PCM, and M30 OGG banks with plain, `nami`, and `0412` encryption.
- Background audio and autoplay keysounds are scheduled inside the audio stream at sample-frame resolution. Manual hits trigger the corresponding sample immediately. Gameplay reads the same audio clock. Pausing freezes both audio and chart time.
- Perfect: ±35 ms. Cool: ±70 ms. Good: ±160 ms. Miss: outside that window. Hold heads and tails count separately; releasing more than 100 ms early misses the tail. Holding until the end completes it automatically.
- Score awards 1,000 / 800 / 500 / 0 points per judgment. Accuracy uses the same weights. Sessions use **no-fail practice**; an empty gauge does not stop playback. Autoplay results are explicitly marked unranked.
- Resizable, fullscreen, HiDPI-enabled native window. Geometry renders directly into the framebuffer, with fonts rasterized at the current display and UI scale. No fixed-resolution image is stretched over the window.
- Bounds-checked binary parsing, bounded file/sample sizes, and worker-thread loading/decoding. Unsupported encryption, invalid offsets, and truncated records produce errors. Legacy M30 banks with overstated record counts are accepted when they end at a complete sample boundary.
- Long-note authoring issues are normalized with warnings: orphan releases become taps, overlapping holds end at the next note, and unclosed hold heads become taps. This keeps other difficulties in the song available for play.

The UI has a fixed logical layout that scales to fit the window. Music selection and saved lane bindings are local; there is no networking, account system, or online ranking.

## Command line

```sh
hyperjam [chart.ojn] \
  --samples bank.ojm \
  --difficulty easy|normal|hard \
  --autoplay --start \
  --speed 1.5 --offset 30 --volume 70
```

`--start` begins playback once loading finishes. `--inspect` parses the chart and decodes the sample bank without opening a window or audio device. Without a chart, it inspects the practice song. `--help` lists the gameplay options.

`--list-music` prints indexed chart metadata. `--validate-music` checks all three difficulty blocks of every indexed chart without decoding audio. Both commands work without a display or audio device.

The UI bundles Noto Sans under its [SIL Open Font License](assets/FONT-LICENSE.txt). It tries common installed CJK fonts for chart metadata. You can explicitly supply a font with Korean, Japanese, or Chinese glyphs:

```sh
hyperjam song.ojn --font /usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc
```

Header strings decode as UTF-8 when valid, otherwise EUC-KR/Windows-949.

For rendering smoke tests, `--screenshot output.png --screenshot-after-seconds 10 --exit-after-seconds 12` captures active gameplay and closes the app. Alternatively use `--screenshot-frame 240 --exit-after 250` to count frames. On Linux it can run inside `xvfb-run`.

## Verify

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Tests use generated binary fixtures rather than copyrighted song files. They cover tempo changes, measure lengths, sample IDs, long-note judgments and legacy corrections, autoplay, sample-frame scheduling, pause behavior, WAV reconstruction/decoding, both M30 masks, sparse-bank record counts, OMC cipher state, case-insensitive bank lookup, truncated files, music search/indexing, and key-binding validation, swaps, and persistence. The supplied `./music` collection has also been checked: all 410 OJN files parse across their three difficulties, and all 417 OJM sample containers parse. This verifies structure, not the playability of every song or the decoding of every audio sample.

Format details were cross-checked against the [Open2Jam OJN parser](https://github.com/open2jamorg/open2jam/blob/master/parsers/src/org/open2jam/parsers/OJNParser.java) and [OJM parser](https://github.com/open2jamorg/open2jam/blob/master/parsers/src/org/open2jam/parsers/OJMParser.java). The OMC permutation is format data from that implementation. The gameplay, renderer, audio scheduler, parsers, and practice song here are written for this project. No O2Jam music or artwork is bundled.
