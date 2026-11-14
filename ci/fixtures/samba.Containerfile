FROM quay.io/archlinux/archlinux:latest
RUN pacman -Sy --noconfirm samba bash && pacman -Scc --noconfirm

# smb.conf must exist before smbpasswd (it loads the passdb backend from it).
RUN printf '%s\n' \
    '[global]' \
    '   workgroup = WORKGROUP' \
    '   server string = harbor spike' \
    '   security = user' \
    '   map to guest = Bad User' \
    '   guest account = nobody' \
    '   log file = /tmp/smbd.log' \
    '   max log size = 5000' \
    '   passdb backend = tdbsam' \
    '   load printers = no' \
    '   printing = bsd' \
    '   printcap name = /dev/null' \
    '   disable spoolss = yes' \
    '[guest]' \
    '   path = /srv/samba/guest' \
    '   read only = yes' \
    '   guest ok = yes' \
    '   browseable = yes' \
    '[private]' \
    '   path = /srv/samba/private' \
    '   read only = yes' \
    '   valid users = harbor' \
    '   browseable = yes' \
    > /etc/samba/smb.conf

# Guest-readable content (passwordless path) and an authenticated share.
RUN mkdir -p /srv/samba/guest/Movies /srv/samba/private /var/lib/samba /run/samba && \
    useradd -M -d /nonexistent -s /usr/sbin/nologin harbor && \
    echo "harbor guest file content 0123456789abcdef" > /srv/samba/guest/hello.txt && \
    seq 1 2000 | tr '\n' ' ' > /srv/samba/guest/big.txt && \
    echo "fake mkv payload for harbor listing" > /srv/samba/guest/movie.mkv && \
    echo "fake episode payload for harbor listing" > /srv/samba/guest/Movies/episode.mkv && \
    echo "secret harbor content 0123456789abcdef" > /srv/samba/private/secret.txt && \
    echo "fake private mkv payload for harbor" > /srv/samba/private/private-movie.mkv && \
    printf 'harborpass\nharborpass\n' | smbpasswd -a -s harbor && \
    smbpasswd -e harbor && \
    chmod -R 0777 /srv/samba

EXPOSE 445
CMD ["/usr/bin/smbd", "--foreground", "--no-process-group", "--debug-stdout"]
