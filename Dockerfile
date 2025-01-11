FROM rust:1.83.0 AS builder

WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./

COPY ./pokemon.json ./pokemon.json
COPY ./db/migrations ./db/migrations
COPY ./screenshots ./screenshots
COPY ./templates ./templates
COPY ./src ./src

RUN cargo build --release

# Start a new stage to create a smaller image without unnecessary build dependencies
FROM debian:bookworm-slim

WORKDIR /usr/src/app

COPY --from=builder /usr/src/app/target/release/pokemon_scraper ./
RUN mkdir db

CMD ["./pokemon_scraper"]

