# File: scripts/lab/make-test-wav.ps1
# Purpose: write a 16 kHz mono 16-bit WAV of synthesized speech ("a portrait of a young woman") as a test sample, so no recorded speech is needed.
# Contents: one synthesizer call that speaks the sentence into the output file.
# Usage: pwsh scripts/lab/make-test-wav.ps1 -Out <file.wav>
param([Parameter(Mandatory)][string]$Out)
Add-Type -AssemblyName System.Speech
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
$synth.SetOutputToWaveFile($Out, (New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(16000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)))
$synth.Speak("A portrait of a young woman.")
$synth.Dispose()
