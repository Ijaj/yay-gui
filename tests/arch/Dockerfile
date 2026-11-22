# Disposable Arch image for the aurdeck harness. Local use only: it carries the
# proxy CA in its trust store when built behind a proxy, and a throwaway sudo
# password. Never push it anywhere. Built by tests/arch/run.sh; see
# tests/arch/README.md.
FROM archlinux:base-devel

# Optional extra trust anchors (an HTTPS-intercepting proxy CA). run.sh copies
# a CA bundle into ca/ when HTTPS_PROXY is set; otherwise the folder holds
# only a README. The proxy's bundle also contains the public roots, which
# `trust anchor --store` rejects as already present, so the files go into the
# anchors directory instead. Any failure fails the build.
COPY ca/ /tmp/ca/
RUN set -e; \
    for f in /tmp/ca/*.crt; do \
      [ -e "$f" ] || continue; \
      cp "$f" /etc/ca-certificates/trust-source/anchors/; \
    done; \
    update-ca-trust; rm -rf /tmp/ca

# Full upgrade at build time (never a partial upgrade). run.sh passes the date
# so a cached layer from an earlier day is never reused for this one.
ARG AURDECK_CACHE_BUST=none
RUN echo "cache key: ${AURDECK_CACHE_BUST}" \
 && pacman -Syu --noconfirm --needed sudo git && pacman -Scc --noconfirm

# Non-root user with a password. sudo always asks for it (no NOPASSWD, no
# cached credentials), so the harness really answers sudo prompts. The password
# is a throwaway kept in a file only dev can read, not in the image environment.
RUN set -e; \
    pw=aurdeck-harness-throwaway; \
    useradd -m dev; \
    echo "dev:${pw}" | chpasswd; \
    printf '%s\n' "$pw" > /etc/aurdeck-harness-sudo-password; \
    chown dev:dev /etc/aurdeck-harness-sudo-password; \
    chmod 0400 /etc/aurdeck-harness-sudo-password; \
    printf 'dev ALL=(ALL) ALL\nDefaults:dev timestamp_timeout=0\nDefaults env_keep += "HTTPS_PROXY https_proxy HTTP_PROXY http_proxy NO_PROXY no_proxy"\n' > /etc/sudoers.d/10-dev; \
    chmod 0440 /etc/sudoers.d/10-dev

# yay from the AUR, built as dev. This is image build, not app code: makepkg
# refuses root, so `su dev -c` is the way to run it as dev here. A temporary
# NOPASSWD rule exists only inside this single RUN so makepkg -s can install
# its dependencies non-interactively. The yay-bin commit is logged.
RUN set -e; \
    printf 'dev ALL=(ALL) NOPASSWD: ALL\n' > /etc/sudoers.d/99-build; \
    chmod 0440 /etc/sudoers.d/99-build; \
    rc=0; \
    su dev -c 'cd ~ && git clone https://aur.archlinux.org/yay-bin.git && cd yay-bin && git rev-parse HEAD > /tmp/yay-bin-commit && makepkg -si --noconfirm' || rc=$?; \
    rm -rf /etc/sudoers.d/99-build /home/dev/yay-bin /home/dev/.cache; \
    [ "$rc" = 0 ] || exit "$rc"; \
    cp /tmp/yay-bin-commit /etc/aurdeck-yay-bin-commit; rm -f /tmp/yay-bin-commit; \
    echo "yay-bin AUR commit: $(cat /etc/aurdeck-yay-bin-commit)"
RUN su dev -c 'yay --version' | tee /etc/aurdeck-yay-version

# Marker the harness requires (root-owned, not writable by others): without it
# the binary refuses to run, so a distrobox or toolbox on a real Arch host is
# not mistaken for this image.
RUN printf 'aurdeck arch harness image\n' > /etc/aurdeck-arch-harness-image \
 && chmod 0644 /etc/aurdeck-arch-harness-image

ENV AURDECK_ARCH_HARNESS=1
USER dev
WORKDIR /home/dev
CMD ["aurdeck-arch-harness", "all"]
