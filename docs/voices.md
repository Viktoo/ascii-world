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

## Not done yet

- Speaking to characters by voice (dictation on `/` or Enter).
- Mood in the voice (calm, scared, angry), from the sim's state.
- Lines the player can't hear yet could be made ahead of time.
- Smaller binary: load the pronunciation dictionaries from the voice pack.
- Host the voice pack on the game's own release.
