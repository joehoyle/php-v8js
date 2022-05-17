# Define versions used to select image versions
# (ARGs declared before FROM can't be used outside of FROMs)
ARG FROM_PHP=8.0

# Select distro
ARG FROM_DISTRO=bullseye

FROM php:${FROM_PHP}-fpm-${FROM_DISTRO}

RUN apt-get update && apt install curl build-essential gcc libclang-dev make -y

RUN curl https://sh.rustup.rs -sSf | bash -s -- -y

RUN echo 'source $HOME/.cargo/env' >> $HOME/.bashrc
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /code
ENTRYPOINT [ "" ]
