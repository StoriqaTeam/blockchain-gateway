FROM debian:stable-slim

ARG env=debug

RUN mkdir -p /app/config \
  && apt-get update \
  && apt-get install -y gnupg2 ca-certificates \
  && apt-get autoremove -y \
  && apt-get clean -y \
  && rm -rf /var/lib/apt/lists/ \
  && adduser --disabled-password --gecos "" --home /app --no-create-home -u 5000 app \
  && chown -R app: /app

COPY target/$env/blockchain_gateway /app
COPY config /app/config

#USER app
WORKDIR /app

ENV PATH=$PATH:/usr/local/cargo/bin/
EXPOSE 8000

ENTRYPOINT ["/app/blockchain_gateway", "server"]
