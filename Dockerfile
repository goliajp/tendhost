# One recipe for the image and for the Linux release binaries.
#
#   docker build -t tendhost .                                   the server image
#   docker build --target artifacts --output type=local,dest=out .   static binaries in out/

FROM oven/bun:1.4-alpine AS web
WORKDIR /src/web
COPY web/package.json web/bun.lock ./
RUN bun install --frozen-lockfile
COPY web ./
RUN bun run build

FROM rust:1.100-alpine AS build
RUN apk add --no-cache musl-dev cmake make perl clang
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY --from=web /src/web/dist web/dist
RUN cargo build --release --locked \
        -p tendhost-server --features tendhost-server/web \
        -p tendhost-hostd -p tendhost-cli

FROM scratch AS artifacts
COPY --from=build /src/target/release/tendhost-server /src/target/release/tendhostd /src/target/release/tendhost /

FROM alpine:3.22
RUN apk add --no-cache ca-certificates \
    && adduser -S -D -H -h /data tendhost \
    && mkdir /data && chown tendhost /data
COPY --from=build /src/target/release/tendhost-server /src/target/release/tendhost /usr/local/bin/
USER tendhost
ENV TENDHOST_DATA=/data TENDHOST_LISTEN=0.0.0.0:7080
VOLUME /data
EXPOSE 7080
ENTRYPOINT ["tendhost-server"]
