# Generates tests/fixtures/jordan.wav: 16 kHz mono PCM with the STT test sentence.
$ErrorActionPreference = 'Stop'
$out = Join-Path (Split-Path -Parent $PSScriptRoot) 'tests/fixtures/jordan.wav'
New-Item -ItemType Directory -Force (Split-Path -Parent $out) | Out-Null

Add-Type -AssemblyName System.Speech
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
$format = [System.Speech.AudioFormat.SpeechAudioFormatInfo]::new(
    16000,
    [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen,
    [System.Speech.AudioFormat.AudioChannel]::Mono)
$synth.SetOutputToWaveFile($out, $format)
$synth.Speak('Good morning everyone. Today we cover sorting. Jordan, what is the complexity of merge sort?')
$synth.Dispose()

Write-Output "wrote $out"
