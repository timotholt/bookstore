FROM rust:1-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --locked -p chantels-corner

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/chantels-corner /app/chantels-corner
COPY --from=builder /app/assets /app/assets
COPY --from=builder /app/app.js /app/app.js
COPY --from=builder /app/styles.css /app/styles.css
USER 65532:65532
CMD ["/app/chantels-corner"]
