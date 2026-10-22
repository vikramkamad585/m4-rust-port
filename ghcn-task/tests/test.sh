#!/usr/bin/env bash
# Evidence only; compute_reward.py decides the score.
set -uo pipefail
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
unset PYTHONPATH PYTHONHOME PYTHONSTARTUP PYTHONUSERBASE LD_PRELOAD LD_LIBRARY_PATH LD_AUDIT \
      BASH_ENV ENV GCONV_PATH PROMPT_COMMAND CDPATH MAKEFLAGS MAKELEVEL MFLAGS CXXFLAGS CFLAGS LDFLAGS
export LC_ALL=C
mkdir -p /logs/verifier

python3 -E -s -I /tests/collect.py || echo "collect.py exited with $?"
exec python3 -E -s -I /tests/compute_reward.py
