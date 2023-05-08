
FROM rust as pipe-build
#repeat ARGs to make them available in the next stage
RUN apt-get update
RUN apt-get install cmake openssl musl-tools -y

WORKDIR /pipe_build
COPY crates ./crates
COPY src ./src
COPY frontend ./frontend
COPY Cargo.toml .
COPY Cargo.lock .

RUN cargo build --release
RUN cargo build --release --all

FROM alpine
WORKDIR pipe_utils
COPY --from=pipe-build /pipe_build/target/release/pipe_downloader /pipe_utils
COPY --from=pipe-build /pipe_build/target/release/pipe_serve /pipe_utils
COPY --from=pipe-build /pipe_build/target/release/pipe_udp_server /pipe_utils
COPY --from=pipe-build /pipe_build/target/release/pipe_mock_serve /pipe_utils
