FROM registry.fedoraproject.org/fedora:latest
RUN dnf install -y minidlna && dnf clean all

RUN mkdir -p /media /var/cache/minidlna /tmp && \
    printf '%s\n' \
    '[general]' \
    '   media_dir=/media' \
    '   friendly_name=Harbor Test DLNA' \
    '   db_dir=/var/cache/minidlna' \
    '   log_dir=/tmp' \
    > /etc/minidlna.conf

EXPOSE 8200 1900/udp
# minidlnad daemonizes; keep PID 1 alive after it starts.
CMD ["sh", "-c", "minidlnad -f /etc/minidlna.conf -d; exec tail -f /dev/null"]
