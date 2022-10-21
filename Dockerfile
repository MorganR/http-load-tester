# syntax=docker/dockerfile:1

### Build the server
FROM rust:bullseye AS builder

WORKDIR /app
COPY . ./

# Compile binary
RUN cargo build --release

FROM debian:bullseye
RUN apt-get update && apt-get install -y --reinstall ca-certificates

WORKDIR /app
COPY --from=builder /app/target/release/http-load-tester ./
ENTRYPOINT ["./http-load-tester"]
