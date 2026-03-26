FROM rust:bookworm

# System dependencies
RUN apt-get update && apt-get install -y \
    nodejs npm \
    chromium \
    && rm -rf /var/lib/apt/lists/*

# Set Chrome path for puppeteer-core
ENV CHROME_PATH=/usr/bin/chromium

WORKDIR /app
COPY . .

# Install mise
RUN curl https://mise.run | sh
ENV PATH="/root/.local/bin:${PATH}"

# Install tools and dependencies
RUN mise trust && mise install
RUN npm install

# Build
RUN cargo build --release

ENTRYPOINT ["./target/release/mdskim"]
