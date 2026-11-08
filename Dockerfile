# The release workflow assembles this context from the verified downloads.
FROM scratch
ARG TARGETARCH
COPY binaries/${TARGETARCH}/pagelet /pagelet
COPY --chown=65532:65532 data /data
COPY LICENSE THIRD_PARTY_NOTICES.md /
COPY web/vendor/*.LICENSE /web/vendor/
ENV PAGELET_DATA_DIR=/data PAGELET_LISTEN=0.0.0.0:8080
USER 65532:65532
EXPOSE 8080
STOPSIGNAL SIGINT
ENTRYPOINT ["/pagelet"]
CMD ["serve"]
