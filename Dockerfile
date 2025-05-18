FROM rust:1.87 AS builder

WORKDIR /usr/src/role-bot
RUN apt-get update && apt-get install -y musl-tools gcc
RUN rustup target add x86_64-unknown-linux-musl

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo fetch
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN rm src/main.rs

COPY src ./src/
RUN touch src/main.rs
RUN cargo install --path . --target x86_64-unknown-linux-musl

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y openssl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/role-bot /usr/local/bin/role-bot

CMD ["role-bot"]
