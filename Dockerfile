# TODO: base image + build for unifi. Mirror jellyfin/Dockerfile conventions.
FROM debian:12-slim
LABEL org.opencontainers.image.source="https://github.com/argyle-labs/unifi"
EXPOSE 8443
