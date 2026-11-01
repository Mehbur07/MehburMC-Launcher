# Builds the Linux package on the base MehburOS uses, so its library versions
# match. Used by scripts/package-linux.mjs.
FROM ubuntu:26.04

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential ca-certificates curl file git patchelf pkg-config \
      librsvg2-dev libssl-dev libwebkit2gtk-4.1-dev \
      nodejs npm \
 && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
ENV PATH=/opt/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal -c clippy -c rustfmt

COPY build.sh /usr/local/bin/build.sh
ENTRYPOINT ["sh", "/usr/local/bin/build.sh"]
