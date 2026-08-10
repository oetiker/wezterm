#!/usr/bin/env bash
# corpus/plain.sh — deterministic text, no animation, no images.
printf 'ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz\n'
printf '0123456789 !"#$%%&()*+,-./:;<=>?@[]^_{|}~\n'
printf 'ligatures: // != == -> <= >= ... =~\n'
printf 'box: \xe2\x94\x8c\xe2\x94\x80\xe2\x94\x90 \xe2\x94\x94\xe2\x94\x80\xe2\x94\x98 blocks: \xe2\x96\x88\xe2\x96\x93\xe2\x96\x92\xe2\x96\x91\n'
printf 'bold: \033[1mBOLD\033[0m italic: \033[3mITALIC\033[0m under: \033[4mUNDER\033[0m\n'
printf 'colors: \033[31mR\033[32mG\033[34mB\033[0m bg: \033[41mR\033[42mG\033[44mB\033[0m\n'
exec sleep 600
