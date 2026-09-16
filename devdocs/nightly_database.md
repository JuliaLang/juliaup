# Nightly catalog

Juliaup consumes VersionsJSONUtil's `bin/nightlies.json` directly through
`JULIAUP_SERVER`. VersionsJSONUtil publishes daily and on manual dispatch; there
is no separate juliaup nightly publishing workflow or deployment prerequisite.

## Selection

The catalog maps nightly base names to `files` (standard artifacts) and `variants`.
Juliaup filters for compatible triplets and tar.gz archives, using the same
nightly platform policy as before catalog discovery. OS and architecture alone
are insufficient to distinguish ABIs. Unknown fields, unsupported platforms and
artifact kinds are ignored. Bad channel names and variant tokens are skipped.
Unparseable JSON cannot replace the cache. Artifact URLs must use HTTPS or
loopback HTTP; official nightly URLs can be redirected by `JULIAUP_NIGHTLY_SERVER`.

Variant names are generic alphanumeric tokens. Sorting their tokens produces
the spelling shown by `list`, such as `nightly+assert+opt`. New installs must use
that exact spelling; permutations and repeated tokens do not create equivalent
installations. Combined variants work only when a matching artifact is published.
The optional `~arch` suffix retains its existing meaning. New variants on known
platforms need no client release; additional ABI/platform rules may need one.

Tarballs remain the catalog selection. On macOS, the downloader first attempts
the derived DMG URL with tarball fallback, as it does for releases. Updates keep
the artifact URL recorded at installation, and use its ETag to detect new builds.
Required nightly/PR ETags are checked in GET headers before processing the body.

## Refresh and cache

- New nightly installations fetch the catalog, falling back to a usable cache
  with a warning when refresh fails.
- Listing and GUI startup/reloads refresh missing or day-old data with a
  five-second total deadline and cached fallback, including for release-only users.
- Background/update checks refresh missing or day-old data only when a nightly
  direct-download channel is installed. PRs, links and aliases alone do not qualify.
- Install/background requests have a 30-second total deadline. Body reads are
  bounded on Windows and Unix. Artifact ETag checks remain independent of catalog
  freshness or refresh failure and the configured update interval is unchanged.
- Launching installed channels and manifest resolution only read local metadata.

`nightlies-cache.json` contains the source URL, successful-fetch timestamp and
raw catalog. It is written atomically after parsing. Source changes invalidate
cached data; failed refreshes do not advance freshness. No hash, version stamp or
conditional GET is involved. Concurrent valid fetches may finish out of order;
atomic replacement prevents partial reads without a separate cache lock.

Mirrors need `bin/nightlies.json`, including for the first nightly install.
Unavailable discovery does not prevent launching already installed channels.
Malformed/missing caches leave release listing available. Manifest selection
retains the historical series-nightly fallback for a too-new patch in a known
series when metadata is unavailable.
