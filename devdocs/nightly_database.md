# Nightly catalog

Juliaup does not construct nightly download URLs itself. Instead, it reads
`bin/nightlies.json` from `JULIAUP_SERVER`, a catalog of nightly builds that
VersionsJSONUtil.jl publishes alongside `versions.json`. The catalog is keyed by
channel (`nightly`, `1.13-nightly`, ...), and for each channel lists the
standard builds (`files`) and the variant builds (`variants`) for every
platform. See `src/nightlies_db.rs`.

## Channel names

A nightly channel name is `<channel>[+<variant>...][~<arch>]`. The variant part
is formed by sorting the variant tokens of a catalog entry, so a build with
variants `["opt", "assert"]` is installed as `nightly+assert+opt`. Juliaup only
accepts that spelling. This keeps channel names opaque identifiers, and
leaves room to move naming to the server in the future.

To select a build, juliaup maps the host platform (or the `~arch` suffix) to the
catalog's `os`/`arch` fields and a fixed list of supported triplets, and picks
the `.tar.gz` archive with exactly the requested variants. On macOS,
`install_from_url` first tries the DMG next to that tarball, as it does for
releases. New variants therefore need no juliaup release; new platforms or ABIs
do.

Catalog entries that cannot be expressed as a channel name are skipped, as are
unknown fields, platforms and file kinds.

## Mirrors

Catalog URLs on the official nightly server are rewritten to
`JULIAUP_NIGHTLY_SERVER` if set; URLs on other hosts (e.g. the `nogpl` builds)
are used as-is. Artifact URLs must use HTTPS, or HTTP on a loopback address.

Nightly and PR channels are updated by comparing the ETag of the artifact URL
recorded at installation, so the catalog is not needed to update an installed
channel. The ETag is checked in the response headers before the download is
extracted.

## Cache and refresh

The catalog is cached in `nightlies-cache.json` in the juliaup directory, together
with the URL it was fetched from and the time of the fetch. A cache from another
URL (e.g. after changing `JULIAUP_SERVER`) is ignored. The cache is only replaced
by a catalog that parses, using an atomic rename, so it does not need the
configuration lock.

The catalog is fetched:

- by `juliaup add` of a nightly channel, always, falling back to the cache with
  a warning if the download fails;
- by `juliaup list` and the GUI, if the cache is missing or older than a day,
  with a 5 second timeout. Without any catalog, generic `nightly` and
  `x.y-nightly` placeholders are listed;
- by `juliaup update` and the periodic background update, if the cache is older
  than a day and a nightly channel is installed. Users without nightlies never
  download the catalog in the background.

Launching Julia never downloads the catalog. When a project manifest requires an
unreleased Julia version, the launcher uses the cached catalog to check whether
the matching `x.y-nightly` channel has a build for this platform, falling back
to `nightly`. Without a cache, it keeps the previous behavior.
