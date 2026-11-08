# Releasing Pagelet

1. Merge the release changes and push a version tag matching `Cargo.toml`, such
   as `v0.1.0`. The **Release** workflow tests each download and image platform.
2. Wait for the tag workflow to succeed. Review the draft release notes,
   downloads, and checksums, then publish the draft as a regular release on
   GitHub. The Docker image is published by the separate release-event run.
3. If Docker Hub publication fails, use **Re-run failed jobs** after correcting
   the error, or use **Run workflow** on the default branch and enter the
   published tag. The workflow fetches `pagelet-image.oci.tar` from the release
   and checks it against `SHA256SUMS`. Both retries use the verified image
   without rebuilding it. Retrying an older release only updates its version tag.

The publishing job uses the repository variable `DOCKERHUB_USERNAME` and the
repository secret `DOCKERHUB_TOKEN`. The token needs push access to
`dreamwalkerxz/pagelet`.
