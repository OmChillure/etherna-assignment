FROM rust:1.86-slim AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY tests ./tests
RUN cargo build --release --bin matcher

FROM debian:bookworm-slim
COPY --from=build /app/target/release/matcher /usr/local/bin/matcher
ENTRYPOINT ["matcher"]
