FROM rust:1.87 AS builder

WORKDIR /usr/src/myapp
COPY . .

RUN apt-get update && apt-get install -y musl-tools gcc

RUN rustup target add x86_64-unknown-linux-musl

RUN cargo install --path . --target x86_64-unknown-linux-musl

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y openssl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/role-bot /usr/local/bin/role-bot

CMD ["role-bot"]
