#!/usr/bin/env sh
# Regenerates the small audio fixtures used by djk-analysis tests. Requires ffmpeg.
# Each file is 1 second of a 440 Hz sine at 44.1 kHz.
set -eu
cd "$(dirname "$0")"
SRC="-f lavfi -i sine=frequency=440:sample_rate=44100:duration=1"
ffmpeg -y -loglevel error $SRC -ac 2 -c:a libmp3lame -b:a 128k -map_metadata -1 tone.mp3
ffmpeg -y -loglevel error $SRC -ac 2 -c:a flac tone.flac
ffmpeg -y -loglevel error $SRC -ac 2 -c:a pcm_s16le tone.wav
ffmpeg -y -loglevel error $SRC -ac 2 -c:a pcm_s16be tone.aiff
ffmpeg -y -loglevel error $SRC -ac 2 -c:a aac -b:a 96k tone.m4a
ffmpeg -y -loglevel error $SRC -ac 1 -c:a pcm_s16le mono.wav
# A file with a supported extension that is not audio.
printf 'this is not an mp3 file at all, just text pretending to be one.\n%.0s' 1 2 3 4 5 6 7 8 > corrupt.mp3
