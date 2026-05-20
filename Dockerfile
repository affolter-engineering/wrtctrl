FROM rust:1.88-bookworm AS builder

RUN apt-get update \
	&& apt-get install -y --no-install-recommends \
		binaryen \
		ca-certificates \
		clang \
		curl \
		lld \
		nodejs \
		npm \
		pkg-config \
	&& rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown
RUN cargo install --locked cargo-leptos wasm-bindgen-cli
RUN npm install -g @tailwindcss/cli

WORKDIR /app

COPY Cargo.toml Cargo.lock build.rs ./
COPY public/ public/
COPY src/ src/
COPY style/ style/

RUN cargo leptos build --release \
	&& ASSAT_ASA_EMBEDDED_JS=target/site/pkg/assat-asa.js \
	   ASSAT_ASA_EMBEDDED_WASM=target/site/pkg/assat-asa.wasm \
	   cargo build --release --no-default-features --features ssr --bin wrtctrl

FROM debian:bookworm-slim

RUN apt-get update \
	&& apt-get install -y --no-install-recommends ca-certificates \
	&& rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/wrtctrl /usr/local/bin/wrtctrl

EXPOSE 3000

CMD ["wrtctrl"]
