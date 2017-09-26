FROM ubuntu:14.04

RUN apt-get update && apt-get install -y \
    curl

RUN mkdir -p /volwork

WORKDIR /volwork

RUN curl https://sh.rustup.rs -sSf > ./rustup.sh && \
    chmod +x ./rustup.sh

COPY vol_mgr.sh ./vol_mgr.sh

ENV HOME=/volwork
RUN chmod 777 /volwork

CMD ["/bin/sh", "./vol_mgr.sh"]
