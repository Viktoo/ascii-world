# Voices

Built 2026-10-13 on the `voice` branch (`src/voice`, `src/app/voices.rs`).
Characters are heard saying their lines, each in a voice of their own, from
where they stand: turn and walk up, and they get clearer and louder, like
every other sound. Everything runs on the player's machine; nothing is sent
anywhere and nothing is paid per line.

## How a line is heard

1. A character says something near the traveler (within 30 m): a chat line,
   a call in a game of catch, or a reply to the player, which is spoken a
   sentence at a time while it is still streaming in.
2. The voice thread turns it into phonemes (`misaki-rs`, dictionary based,
   no eSpeak), and words no dictionary knows, mostly the world's own
   names, are sounded out by `names.rs` instead of spelled letter by letter.
3. **Kokoro** (82M parameters, Apache 2.0, through ONNX Runtime) speaks
   them in the character's blend of two model voices.
4. **The body** (`body.rs`, the WORLD vocoder): the speech is split into
   pitch, the throat's resonances and breath, and each is reshaped (pitch and
   its range, throat size, breathiness, rasp, an old voice's tremor) before
   it is put back together.
5. The clip plays through the mixer as a placed voice that follows the
   speaker. A speaker's next sentence waits for the last one to end.

## A character's voice (`recipe.rs`)

Read off the persona, with no extra LLM call:

- **Sex** from how they are described (woman, fisherman, grandmother, beard…).
- **Age**: children small and high, the old slower, shakier, rougher, breathier.
- **Height and build**: taller means a longer throat and a lower voice.
- **How they speak** (the persona's `voice` line): slow or quick, gruff, soft,
  deep, shrill, flat, lilting, nervous each move a slider.
- **Their own**: two model voices of the right kind (US or British, so the
  pronunciation matches) in a mix of their own, and a small offset from
  their id. The same character always sounds the same.

### Tone: numbers the LLM writes

Over that guess, a character's `tone` (and their species' `tone` under
it) says outright how they sound, as numbers the world's writer gives,
like the sliders of animal calls: `pitch` (Hz), `range`, `size` (the
throat), `pace`, and 0–1 `breath`, `rough`, `whisper`, `under`, `hollow`,
`waver` (with `waver_hz`). All are done by the body, after the model has
spoken, on WORLD's parts of the speech: a **whisper** drives the mouth with
breath and no voice (aperiodicity up; past 0.75 no pitch at all), **under**
synthesises the same speech again an octave down from a bigger throat and
mixes it in, **hollow** combs the throat's resonances every 550 Hz and adds
a short 9 ms ring, **waver** is the tremor at a chosen rate. The phantom's
species carries a stock tone like its stock body. `pocket listen --voices`
says each one in the cast, the phantom among them.

The sim spaces a conversation's lines by how long each takes to say at the
speaker's pace (`voice::secs`), heard or not, so talk keeps a human rhythm.

## The voice pack

`~/.pocket/voice` (or `POCKET_VOICE_DIR`): `kokoro-v1.0.onnx` (325 MB) and
`voices-v1.0.bin` (28 MB), fetched once the first time voices are on (from
the kokoro-onnx project's GitHub release). The game binary grows by about
65 MB: ONNX Runtime, linked in, and the pronunciation dictionaries.

## Settings and tools

- Settings: **Voices** on/off (needs Sound). `POCKET_NO_VOICE` turns them off.
- `pocket listen --voices [FILE] [--say TEXT] [--fetch]`: a built-in cast, or
  a world's speaking people, each saying a line, as WAVs, with their recipes
  and how long each took to make.
- `POCKET_VOICE_HARVEST=1`: the slower, steadier WORLD pitch tracker, to
  compare against the default (DIO).

## Speed

On an M5 Pro with 4 threads (a quarter of the cores, at most 4), a 4-second
line takes about 1 second to make: the model 0.8 s, the body 0.15 s. Frame
rate is the same with voices on and off.

## Speaking to talk

Built 2026-10-13 (`src/ears`, `src/app/ears.rs`). Press `/`, or Enter with
someone, and the mic opens: the words show in the input line as they are made
out. A pause (1 s) or Enter closes the mic, and the words wait in the line
to be read over and edited; only Enter sends them, as with typed text, so
nothing heard goes out unseen. Typing (Backspace too) takes over at once (the
mic closes and the words stay to edit), Esc forgets what was heard. With someone, Enter on an empty line
listens again. The mic is open only while listening.

- **Recogniser**: whisper.cpp (`whisper-rs`, MIT / Unlicense; the model
  weights are OpenAI's, MIT), `ggml-base.en.bin` (148 MB), fetched once into
  the voice folder. On the CPU with the voices' share of threads: words
  every 0.4 s take 30-80 ms (only as much audio as was said), the whole line
  at the pause about 0.3 s. The names of who you're with, what you point at
  and what you hold are given as a prompt, so they come out spelled right.
  Metal works too (`POCKET_EARS_GPU=1`, 65 ms for the line) but the GPU is
  the world's; frame rate is the same with the CPU recogniser running.
  `small.en` (q8, 265 MB) gets live words right sooner but takes 1 s for the
  line on the CPU.
- **Why not the cloud**: OpenAI's or Deepgram's streaming recognisers are
  more accurate on odd words and cost little (under a cent a minute), but
  need a key, send the player's voice away, and add the network to every
  line. On the machine fits how voices are made.
- **Pause** (`pause.rs`): loudness against the room's noise (the quietest
  frame of the last 1.5 s), so a fan or rain doesn't keep it open. Nothing
  said for 8 s closes it; one go is at most 28 s.
- `cmake` is needed to build (whisper.cpp): `brew install cmake`.
- Settings: **Speak to talk**, off by default (`/` is then just the command line). Turning it on opens the mic once so the system asks for it right away, and says whether it hears. `POCKET_NO_MIC` turns it off.
- `pocket listen --hear F.wav [--model PATH] [--gpu]`: a recording heard as
  in the game, each pass timed. `pocket listen --mic`: one go through the
  microphone. `POCKET_MIC_WAV=F.wav`: the game hears that file as the mic
  (for `--shot` runs: `--keys / --keys-at 8`).
- On a Mac the system asks once for the microphone, for the app the game was
  started from (the terminal); until it is allowed, the line says it is
  waiting for the microphone.

## Not done yet

- Mood in the voice (calm, scared, angry), from the sim's state.
- Lines the player can't hear yet could be made ahead of time.
- Smaller binary: load the pronunciation dictionaries from the voice pack.
- Host the voice pack on the game's own release.
