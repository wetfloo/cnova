cnova
-----
cnova is a command-line oriented tool to quickly obtain lyrics files for your music
by going through the provided paths recursively. It uses [LRCLIB](https://lrclib.net/) API to do so

Installation
------------
If you have Rust installed, simply execute
```
cargo install cnova
```
to obtain the latest release version. Binaries are also available on
[releases](https://github.com/wetfloo/cnova/releases/latest) page.

Usage
-----
In a typical usage scenario, you probably want to download lyrics for your music.
To do so, simply point `cnova` to the directory with music, like so:
```
cnova ~/Music
```

`cnova` also supports downloading lyrics for single tracks, like so:
```
cnova ~/Music/track1.flac ~/Music/track2.flac
```

You can also mix and match directories and files.

When `cnova` obtains lyrics for a song, whether synced or unsynced, it will
create a corresponding `lrc`, if not present. For example, upon download lyrics
for a file `~/Music/track1.flac`, `Music/track1.lrc` will be created. Optionally,
you can force `cnova` to re-download `lrc` files, even if such files present

If `cnova` is unable to obtain lyrics for a song
(for example, LRCLIB returns 404, or claims that a given track is instrumental),
it will create an empty `nolrc` file, corresponding to a given track.
If such file is encountered in the future, `cnova` won't attempt to download lyrics again,
unless specified.

TODOS
-----
- [ ] Progress bars, ETAs
- [ ] Caching for lyrics (to not re-download them every time, and to potentially avoid filling the filesystem with a bunch of empty .nolrc files)
- [ ] Better tracing in general (levels that make more sense, more informational and, at the same time, less noisy error messages)
- [ ] Testing, somehow. Nicely separated functions instead of this mess
