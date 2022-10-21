# syntax=docker/dockerfile:1

### Build the server
FROM rust:bullseye AS builder

WORKDIR /app
COPY . ./

# Compile binary
RUN cargo build --release

FROM debian:bullseye
WORKDIR /app
COPY --from=builder /app/target/release/http-load-tester ./
ENTRYPOINT ["./http-load-tester"]
