# 零依赖，所以不需要缓存 cargo registry，直接一把编
FROM rust:1.95-slim AS build
WORKDIR /src
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=build /src/target/release/mazu /usr/local/bin/mazu
ENV PORT=8080 HOST=0.0.0.0 MAZU_DATA_DIR=/data
VOLUME /data
EXPOSE 8080
CMD ["mazu"]
