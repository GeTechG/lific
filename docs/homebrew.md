# Homebrew tap

Lific ships to Homebrew through a tap, `VoidNullable/homebrew-tap`. Users install
with:

```bash
brew install voidnullable/tap/lific
```

The formula installs the prebuilt release binaries (macOS arm64 and x86_64,
Linux arm64 and x86_64), pinned to the hashes in the release's
`sha256sums.txt`. It does not compile anything, so an install takes seconds and
needs no Rust toolchain. It also installs bash, zsh and fish completions from
`lific completion`.

Requested in GitHub issue #93.

## What lives where

- **`scripts/render-homebrew-formula.sh`** (this repo) prints the formula for a
  version from a `sha256sums.txt`. The formula is generated, never edited by
  hand: change the script.
- **`scripts/render-homebrew-formula.test.sh`** checks the script. It runs in
  `lific:release-test`, which is part of `lific:check`.
- **The `homebrew` job in `.github/workflows/release.yml`** runs after the
  release job on every tag. It downloads the published `sha256sums.txt`,
  renders `Formula/lific.rb`, commits it to a checkout of the tap, installs and
  tests it with Homebrew on the Linux runner, and only then pushes.
- **`VoidNullable/homebrew-tap`** holds `Formula/lific.rb`, and nothing else
  needs to be in it.

The job is non-fatal (`continue-on-error: true`). The binaries are already
published when it runs, so a failure leaves the tap one version behind rather
than failing the release. Without the `HOMEBREW_TAP_DEPLOY_KEY` secret it skips
with a notice.

## One-time setup

1. Create a **public** repository named `VoidNullable/homebrew-tap`, initialized
   with a README so it has a `main` branch. The `homebrew-` prefix is what lets
   users type `voidnullable/tap`.
2. Make a deploy key pair. Keep no copy of the private key once step 4 is done.

   ```bash
   ssh-keygen -t ed25519 -N "" -C "lific release workflow" -f homebrew-tap-deploy
   ```

3. In `VoidNullable/homebrew-tap` → Settings → Deploy keys → Add deploy key:
   title `lific release workflow`, paste `homebrew-tap-deploy.pub`, and tick
   **Allow write access**.
4. In `VoidNullable/lific` → Settings → Secrets and variables → Actions → New
   repository secret: name `HOMEBREW_TAP_DEPLOY_KEY`, value the whole contents
   of `homebrew-tap-deploy` (the private key, including the `BEGIN` and `END`
   lines). Then delete both key files.
5. Seed the tap with the current release so `brew install` works before the
   next tag:

   ```bash
   version=2.11.1   # the latest release
   gh release download "v$version" --repo VoidNullable/lific --pattern sha256sums.txt
   git clone git@github.com:VoidNullable/homebrew-tap.git
   mkdir -p homebrew-tap/Formula
   bash scripts/render-homebrew-formula.sh "$version" sha256sums.txt > homebrew-tap/Formula/lific.rb
   git -C homebrew-tap add Formula/lific.rb
   git -C homebrew-tap commit -m "lific $version"
   git -C homebrew-tap push
   ```

6. Check it: `brew install voidnullable/tap/lific && lific --version`.

A deploy key is used instead of a personal access token because it can write to
the tap repository and nothing else, and it does not expire.

## Re-running a failed update

Re-run the `homebrew` job from the release's workflow run in the Actions tab.
It is idempotent: if `Formula/lific.rb` already matches the release, it commits
nothing and the push is a no-op. To do it by hand, follow step 5 with the
version you want.

## Linux and the background service

On Linux the service that `lific init` installs records the binary's resolved
path, which under Homebrew is the versioned keg
(`.../Cellar/lific/<version>/bin/lific`). `brew upgrade` removes the old keg, so
the formula prints a caveat on Linux telling users to run `lific service install`
after upgrading. macOS records the `bin/lific` link and is not affected.

## homebrew-core

Lific could later be submitted to homebrew-core, which would make it
`brew install lific` with no tap. homebrew-core builds from source, so that
formula would compile the crates.io package (which carries the built web UI)
with Rust rather than install release binaries, and Homebrew's maintainers would
take over version bumps. Its acceptance rules for notability and self-submitted
projects apply, so it is a separate decision from the tap.
