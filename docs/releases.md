# Releases

The game is called **Infinite Pocket** to players. Its worlds stay `.pocket`
files and the command stays `pocket`.

## The release number

The `version` in `Cargo.toml` is the release. A build is that release when it
is a clean build of the commit tagged `v<version>`; any other build is `dev`
(`build.rs` sets `POCKET_RELEASE`, read as `db::RELEASE`). Players see
`0.1.0`, or `dev <commit>`, on the worlds list and in the settings menu.

A world keeps the release and commit that made it (`made_with_release`,
`made_with`) and that last opened it (`last_opened_with_release`,
`last_opened_with`).

Playing together needs the same release on both sides: the host and each
guest compare `db::RELEASE`, `db::COMMIT` and the network protocol when a
guest joins, and the one who is behind is told to update.

## Cutting a release

1. Bump `version` in `Cargo.toml` (`0.2.0`), and run `cargo build` so
   `Cargo.lock` follows.
2. `cargo test --release --features slow-tests`.
3. Commit (`Release 0.2.0`), then tag and push the tag:

```bash
git tag v0.2.0
```

```bash
git push origin v0.2.0
```

The `release` workflow (`.github/workflows/release.yml`) builds
`Infinite Pocket.app` for Apple Silicon on the tag, zips it, and attaches it
to a GitHub release for that tag.

## Opening an unsigned build

Until the app is signed and notarized (an Apple Developer account), macOS
says it can't check it. Right-click the app and choose Open, then Open
again; after that it opens normally.

## Updates

On start the worlds list asks GitHub once for the latest release and says
when a newer one is out (settings: "Check for updates"). It never updates
by itself.

## Later

- Signing and notarizing.
- Intel Mac, Windows and Linux builds.
