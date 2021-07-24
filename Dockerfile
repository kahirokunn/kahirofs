FROM rust
RUN apt-get update -y && \
  apt upgrade -y && \
  apt-get install -y \
  fuse libfuse-dev kmod psmisc vim
