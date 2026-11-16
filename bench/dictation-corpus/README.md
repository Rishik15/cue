# Personal dictation corpus

Speech decisions (thread count, hotwords for names, pause and chunk tuning) are judged on your own voice and vocabulary, so the numbers do not come from read audiobooks. The corpus stays on your machine: it is never committed, and the bench never prints a transcript.

## What to record

At least 30 clips, 3 to 30 seconds each, spoken the way you dictate into Cue. Cover:

- People and company names you actually write (including ones a model is likely to spell wrong).
- Numbers: amounts, dates, times, phone numbers, version numbers.
- Technical terms: product, library and command names, acronyms.
- A mix of short messages and longer paragraphs, with natural pauses and a few false starts.

Record in your usual room and mic. Do not read from a script word for word if you would not dictate that way, but the reference must match what you said.

## Files

One folder, for example `%USERPROFILE%\cue-lab\my-set`:

- `NN.wav`: 16 kHz, mono, 16-bit PCM (Audacity: set Project Rate 16000, Mono, export WAV signed 16-bit PCM). Other formats are rejected.
- `refs.tsv`: one line per clip, tab separated: `id<TAB>reference text<TAB>protected words`. `id` is the file name without `.wav`. Protected words are the names, numbers and terms that must survive, separated by `|` (for example `Priya Raman|4500|kubectl`).

## Check it, then run it

```
CUE_CORPUS=<folder> scripts/cue.ps1 lab corpus      # counts only: clips, seconds, protected words, problems
CUE_BENCH_SET=<folder> CUE_BENCH_RAW=1 scripts/cue.ps1 lab speech
```

`CUE_BENCH_RAW` uses each clip as it is (no joining) and reports word error rate and protected words lost per model.
