#!/bin/sh
# usage: measure.sh CODEC GROUP...   (inside container, cwd = /w)
codec=$1; shift
for g in "$@"; do
  o=0; c=0; bad=0; t0=$(date +%s.%N)
  for f in raw/$g/*.dly; do
    $codec compress $f /tmp/c.bin && $codec decompress /tmp/c.bin /tmp/d.out || { bad=$((bad+1)); continue; }
    cmp -s $f /tmp/d.out || bad=$((bad+1))
    o=$((o + $(stat -c %s $f))); c=$((c + $(stat -c %s /tmp/c.bin)))
  done
  t1=$(date +%s.%N)
  python3 -c "print('%-13s ratio=%.2fx  mismatches=%d  secs=%.1f' % ('$g', $o/max(1,$c), $bad, $t1-$t0))"
done
