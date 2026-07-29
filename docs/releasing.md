# Releasing VibeCon

VibeCon uses [Semifold](https://github.com/noctisynth/semifold) for changeset
intent and for keeping the Node and Rust manifests in step. It is a desktop
app, so neither manifest is published to npm or crates.io.

## Add a changeset

Install `smif` from the Semifold release you want to use, then create a small
Markdown file under `.changes/`:

```md
---
vibecon-web: patch:fix
vibecon-desktop: patch:fix
---
Explain the user-visible change.
```

Use `minor:feat` for a new user-facing capability. The package ids are the
explicit keys in `.changes/config.toml`, not the duplicated `vibecon` names in
the Node and Cargo manifests.

## Cut a release

1. Start from a clean `main` checkout and run `smif status`.
2. Run `smif version`; review the changes to `package.json`,
   `src-tauri/Cargo.toml`, and generated changelogs.
3. Run `pnpm sync:tauri-version`. Semifold has Node and Rust resolvers, but
   Tauri's JSON bundle manifest is deliberately synchronized by this explicit
   script.
4. Verify with `pnpm build` and `cargo check --manifest-path src-tauri/Cargo.toml`.
5. Commit the version bump and tag it as `v<version>`. The GitHub release
   workflow builds the macOS DMG from that tag.

Never use `smif publish` for VibeCon's app release: package registry publishing
is intentionally disabled in `.changes/config.toml`.
