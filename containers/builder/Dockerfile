ARG parent_image
FROM ${parent_image}

# rust toolchain

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    CARGO_TARGET_DIR=/out \
    PATH=/usr/local/cargo/bin:$PATH
VOLUME [ "/out" ]
VOLUME [ "/usr/local/cargo/registry" ]

RUN install_packages \
        ca-certificates \
        curl \
        gcc \
        make \
        libc6-dev \
        wget \
        xz-utils \
        ; \
    \
    url="https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init"; \
    wget "$url"; \
    chmod +x rustup-init; \
    ./rustup-init -q -y --no-modify-path --default-toolchain stable; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
    rustup --version; \
    cargo --version; \
    rustc --version;

# wasm-pack && wasm32 target

RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh; \
    rustup target add wasm32-unknown-unknown

# node/npm

ENV NODE_VERSION 12.16.2

RUN wget https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz; \
    tar xf node-v${NODE_VERSION}-linux-x64.tar.xz -C /opt; \
    mv /opt/node-v${NODE_VERSION}-linux-x64/bin/* /usr/local/bin/; \
    mv /opt/node-v${NODE_VERSION}-linux-x64/include/* /usr/local/include/; \
    mv /opt/node-v${NODE_VERSION}-linux-x64/lib/* /usr/local/lib/; \
    mv /opt/node-v${NODE_VERSION}-linux-x64/share/doc /usr/local/share/; \
    mv /opt/node-v${NODE_VERSION}-linux-x64/share/man/man1 /usr/local/share/man/; \
    mv /opt/node-v${NODE_VERSION}-linux-x64/share/systemtap /usr/local/share/; \
    rm -rf /opt/node-v${NODE_VERSION}-linux-x64; \
    rm -f node-v${NODE_VERSION}-linux-x64.tar.xz

# node utilities

RUN npm install -g rollup@2.6.1 uglify-es@3.3.9

# our dependencies

RUN install_packages \
        libcurl4-openssl-dev \
        libssl-dev \
        libssl1.1 \
        openssl \
        pkg-config \
        ssl-cert \
        ; \
    cargo install cargo-deb; \
    rm -rf ${CARGO_HOME}/registry/*

WORKDIR /src
VOLUME ["/src"]
