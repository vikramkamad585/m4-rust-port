# m4rs workspace: Rust toolchain (offline), Python for the local tools.
FROM rust:1.83.0-slim-bookworm@sha256:540c902e99c384163b688bbd8b5b8520e94e7731b27f7bd0eaa56ae1960627ab

ENV DEBIAN_FRONTEND=noninteractive \
    PAGER=cat \
    GIT_PAGER=cat \
    LC_ALL=C.UTF-8

RUN apt-get update \
 && apt-get install -y --no-install-recommends python3 curl ca-certificates git procps less \
 && (apt-get purge -y m4 bison flex >/dev/null 2>&1 || true) \
 && rm -rf /var/lib/apt/lists/* \
 && ! command -v m4

# Agent CLIs: Claude Code and Codex
RUN curl -fsSL https://claude.ai/install.sh | bash
ENV PATH="/root/.local/bin:${PATH}"
RUN apt-get update \
 && apt-get install -y --no-install-recommends nodejs npm \
 && rm -rf /var/lib/apt/lists/* \
 && npm install -g @openai/codex@0.163.0 \
 && npm cache clean --force \
 && codex --version

# Unprivileged account used to execute the built binary during grading.
RUN useradd --system --no-create-home --shell /usr/sbin/nologin m4grade

WORKDIR /app
COPY README.md /app/README.md
COPY m4rs /app/m4rs
COPY docs /app/docs
COPY examples /app/examples
COPY tools /app/tools
RUN chmod +x /app/tools/*.py \
 && cd /app/m4rs && cargo build --release --offline \
 && git config --global user.email dev@localhost && git config --global user.name dev \
 && cd /app && git init -q && git add -A && git commit -qm "workspace snapshot"

# --- task timer (append at the end of environment/Dockerfile) ---
# COPY only — never auto-start it from the image (no ENV BASH_ENV, no shell
# profile hooks). The platform arms /app/timer.sh in the task sandbox with the
# real TASK_BUDGET_SECS. An image-side autostart also fires inside the image
# BUILD container and stalls the build until the timer's budget expires.
COPY timer.sh /app/timer.sh
RUN chmod +x /app/timer.sh && mkdir -p /app/.timer
