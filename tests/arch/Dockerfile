# Disposable Arch image for the aurdeck harness. Never used outside tests.
# Built by tests/arch/run.sh; see tests/arch/README.md.
FROM archlinux:base-devel

# Optional extra trust anchors (an HTTPS-intercepting proxy CA). run.sh copies
# a CA bundle into ca/ when HTTPS_PROXY is set; otherwise the folder holds
# only a README.
COPY ca/ /tmp/ca/
RUN for f in /tmp/ca/*.crt; do \
      [ -e "$f" ] && trust anchor --store "$f"; \
    done; \
    update-ca-trust; rm -rf /tmp/ca

# Full upgrade at build time (never a partial upgrade), then the tools yay needs.
RUN pacman -Syu --noconfirm --needed sudo git && pacman -Scc --noconfirm

# Non-root user with a password. sudo always asks for it (no NOPASSWD, no
# cached credentials), so the harness really answers sudo prompts.
ENV AURDECK_HARNESS_SUDO_PASSWORD=aurdeck-harness-throwaway
RUN useradd -m dev \
 && echo "dev:${AURDECK_HARNESS_SUDO_PASSWORD}" | chpasswd \
 && printf 'dev ALL=(ALL) ALL\nDefaults:dev timestamp_timeout=0\nDefaults env_keep += "HTTPS_PROXY https_proxy HTTP_PROXY http_proxy NO_PROXY no_proxy"\n' > /etc/sudoers.d/10-dev \
 && chmod 0440 /etc/sudoers.d/10-dev

# yay from the AUR, built as dev. A temporary NOPASSWD rule exists only inside
# this single RUN so makepkg -s can install its dependencies non-interactively.
RUN printf 'dev ALL=(ALL) NOPASSWD: ALL\n' > /etc/sudoers.d/99-build \
 && chmod 0440 /etc/sudoers.d/99-build \
 && { su dev -c 'cd ~ && git clone https://aur.archlinux.org/yay-bin.git && cd yay-bin && makepkg -si --noconfirm'; rc=$?; \
      rm -rf /etc/sudoers.d/99-build /home/dev/yay-bin /home/dev/.cache; exit $rc; }
RUN yay --version | tee /etc/aurdeck-yay-version

ENV AURDECK_ARCH_HARNESS=1
USER dev
WORKDIR /home/dev
CMD ["aurdeck-arch-harness", "all"]
