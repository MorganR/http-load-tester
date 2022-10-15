# syntax=docker/dockerfile:1

### Build the server
FROM golang:1.19-alpine AS builder

WORKDIR /app
COPY . ./

# Compile binary
RUN go mod download && go mod verify
RUN go build -o /stresser .

FROM alpine:latest
WORKDIR /app
COPY --from=builder /stresser ./
ENTRYPOINT ["./stresser"]
