# GHCN-Daily codec workspace: C/C++ toolchain, compression libraries, Python.
FROM debian:bookworm-20241016@sha256:e11072c1614c08bf88b543fcfe09d75a0426d90896408e926454e88078274fcb

ENV DEBIAN_FRONTEND=noninteractive \
    PAGER=cat \
    GIT_PAGER=cat \
    LC_ALL=C.UTF-8

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      g++ make zlib1g-dev liblzma-dev libzstd-dev libbz2-dev \
      python3 xz-utils zstd bzip2 git curl ca-certificates procps less nodejs npm \
 && rm -rf /var/lib/apt/lists/*

# Agent CLIs
RUN curl -fsSL https://claude.ai/install.sh | bash
ENV PATH="/root/.local/bin:${PATH}"
RUN npm install -g @openai/codex@0.163.0 && npm cache clean --force && codex --version

# Accounts used to build and run the codec during grading.
RUN for u in codecbuild codecpack codecunpack; do useradd --system --no-create-home --shell /usr/sbin/nologin $u; done

WORKDIR /app
COPY codec /app/codec
COPY data /app/data
COPY docs /app/docs
COPY tools /app/tools
RUN chmod +x /app/tools/*.py \
 && make -C /app/codec \
 && git config --global user.email dev@localhost && git config --global user.name dev \
 && printf 'codec/codec\n*.o\n.timer/\n' > /app/.gitignore \
 && cd /app && git init -q && git add -A && git commit -qm "initial workspace"

# --- task timer (append at the end of environment/Dockerfile) ---
# COPY only — never auto-start it from the image (no ENV BASH_ENV, no shell
# profile hooks). The platform arms /app/timer.sh in the task sandbox with the
# real TASK_BUDGET_SECS. An image-side autostart also fires inside the image
# BUILD container and stalls the build until the timer's budget expires.
COPY timer.sh /app/timer.sh
RUN chmod +x /app/timer.sh && mkdir -p /app/.timer
