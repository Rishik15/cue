# Purpose: write a 16 kHz mono 16-bit WAV of synthesized speech ("a portrait of a young woman") for the speech worker test, so it needs no recorded sample.
# Usage: pwsh scripts/make-test-wav.ps1 -Out C:\dev\cue-test.wav
param([Parameter(Mandatory)][string]$Out)
Add-Type -AssemblyName System.Speech
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
$synth.SetOutputToWaveFile($Out, (New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(16000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)))
$synth.Speak("A portrait of a young woman.")
$synth.Dispose()
