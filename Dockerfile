FROM rust:1-slim AS build
RUN rustup target add x86_64-unknown-linux-musl && \
    apt-get update && apt-get install -y musl-tools && rm -rf /var/lib/apt/lists/*
ENV CC_x86_64_unknown_linux_musl=musl-gcc \
    CC_x86_64-unknown-linux-musl=musl-gcc
WORKDIR /app
# Release version baked into the binary; empty => falls back to Cargo.toml.
ARG STARS_VERSION=""
ENV STARS_VERSION=${STARS_VERSION}
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN cp target/x86_64-unknown-linux-musl/release/stars /stars

FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /stars /stars
ENV BIND_ADDR=0.0.0.0:8080 DATABASE_URL=sqlite:///data/stars.db
EXPOSE 8080
VOLUME ["/data"]
# Numeric uid:gid (distroless "nonroot") so the kubelet can verify the image
# runs as non-root without a pod-level runAsUser.
USER 65532:65532
ENTRYPOINT ["/stars"]
