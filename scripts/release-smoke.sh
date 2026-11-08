#!/usr/bin/env bash
set -Eeuo pipefail

mode=$1
target=$2
work=$(mktemp -d)
vendor=$PWD/web/vendor/purify.js
password='release smoke password'
container=
volume=
pid=

cleanup() {
    status=$?
    if [[ "$status" != 0 && -f "$work/service.log" ]]; then
        cat "$work/service.log" >&2
    fi
    if [[ -n "$pid" ]]; then
        kill -INT "$pid" 2>/dev/null || true
        wait "$pid" || true
    fi
    if [[ -n "$container" ]]; then
        docker rm -f "$container" >/dev/null 2>&1 || true
    fi
    if [[ -n "$volume" ]]; then
        docker volume rm "$volume" >/dev/null 2>&1 || true
    fi
    rm -rf "$work"
}
trap cleanup EXIT
trap 'echo "Release smoke check failed at line $LINENO" >&2' ERR

wait_for_reader() {
    for ((i = 0; i < 100; i++)); do
        if curl -fs --max-time 2 "$url/_/openapi.json" >/dev/null; then
            return
        fi
        sleep 0.1
    done
    echo "Pagelet did not become ready at $url" >&2
    if [[ -n "$container" ]]; then
        docker logs "$container" >&2
    else
        cat "$work/service.log" >&2
    fi
    return 1
}

publish_and_read() {
    curl -fsS -c "$work/cookies" -H 'x-pagelet-csrf: 1' -H 'Content-Type: application/json' \
        --data "{\"username\":\"smoke-admin\",\"password\":\"$password\"}" "$url/_/api/v1/login" >/dev/null
    curl -fsS -b "$work/cookies" -H 'x-pagelet-csrf: 1' -H 'Content-Type: application/json' \
        --data '{"slug":"release-smoke","markdown":"# Published"}' "$url/_/api/v1/pages" >/dev/null
    curl -fsS "$url/release-smoke" >/dev/null
    curl -fsS "$url/_/assets/vendor/purify.js" | cmp - "$vendor"
    read_page
}

read_page() {
    [[ $(curl -fsS "$url/_/raw/release-smoke") == '# Published' ]]
}

start_container() {
    container=$(docker run -d --platform "linux/$arch" -p 127.0.0.1::8080 -v "$volume:/data" "$@" "$target")
    url="http://$(docker port "$container" 8080/tcp)"
    wait_for_reader
}

stop_container() {
    docker stop "$container" >/dev/null
    [[ $(docker inspect --format '{{.State.ExitCode}}' "$container") == 0 ]]
    docker logs "$container" >"$work/service.log" 2>&1
    grep -q 'event=shutdown_completed' "$work/service.log"
    docker rm "$container" >/dev/null
    container=
}

case "$mode" in
    archive)
        unset RUST_LOG PAGELET_ADMIN_USERNAME PAGELET_ADMIN_PASSWORD
        tar -xzf "$target" -C "$work"
        binary="$work/pagelet"
        if [[ $(uname -s) == Linux ]]; then
            readelf -lW "$binary" >"$work/elf-segments"
            readelf -dW "$binary" >"$work/elf-dynamic"
            if grep -Eq 'INTERP|\(NEEDED\)' "$work"/elf-*; then
                echo 'The Linux executable has a dynamic dependency' >&2
                exit 1
            fi
        fi
        cd "$work"
        printf '%s\n' smoke-admin "$password" "$password" | "$binary" init >/dev/null
        "$binary" serve --listen 127.0.0.1:18972 >"$work/service.log" 2>&1 &
        pid=$!
        url=http://127.0.0.1:18972
        wait_for_reader
        publish_and_read
        kill -INT "$pid"
        wait "$pid"
        pid=
        grep -q 'event=shutdown_completed' "$work/service.log"
        ;;
    image)
        arch=$3
        [[ $(docker image inspect --format '{{.Architecture}}' "$target") == "$arch" ]]
        [[ $(docker image inspect --format '{{.Config.User}}' "$target") == 65532:65532 ]]
        volume=$(docker volume create)
        start_container -e PAGELET_ADMIN_USERNAME=smoke-admin -e "PAGELET_ADMIN_PASSWORD=$password"
        publish_and_read
        stop_container
        grep -q 'event=administrator_initialized source=environment' "$work/service.log"
        start_container
        read_page
        stop_container
        ;;
    *) echo "Expected archive or image mode" >&2; exit 2 ;;
esac
