# recrs workspace: offline Rust toolchain plus Python for the sample checker.
FROM rust:1.83.0-slim-bookworm@sha256:540c902e99c384163b688bbd8b5b8520e94e7731b27f7bd0eaa56ae1960627ab

ENV DEBIAN_FRONTEND=noninteractive \
    PAGER=cat \
    GIT_PAGER=cat \
    LC_ALL=C.UTF-8

RUN apt-get update \
 && apt-get install -y --no-install-recommends python3 curl ca-certificates git procps less nodejs npm \
 && rm -rf /var/lib/apt/lists/* \
 && ! command -v recsel

# Agent CLIs
RUN curl -fsSL https://claude.ai/install.sh | bash
ENV PATH="/root/.local/bin:${PATH}"
RUN npm install -g @openai/codex@0.163.0 && npm cache clean --force && codex --version

# Unprivileged account that runs the built binary during grading.
RUN useradd --system --no-create-home --shell /usr/sbin/nologin recgrade

WORKDIR /app
COPY NOTES.md /app/NOTES.md
COPY recrs /app/recrs
COPY reference /app/reference
COPY samples /app/samples
COPY check.py rules.py /app/
RUN chmod +x /app/check.py /app/rules.py \
 && cd /app/recrs && cargo build --release --offline \
 && git config --global user.email dev@localhost && git config --global user.name dev \
 && cd /app && git init -q && git add -A && git commit -qm "initial workspace"

# --- task timer (append at the end of environment/Dockerfile) ---
# COPY only — never auto-start it from the image (no ENV BASH_ENV, no shell
# profile hooks). The platform arms /app/timer.sh in the task sandbox with the
# real TASK_BUDGET_SECS. An image-side autostart also fires inside the image
# BUILD container and stalls the build until the timer's budget expires.
COPY timer.sh /app/timer.sh
RUN chmod +x /app/timer.sh && mkdir -p /app/.timer
