# freearc-native test results

Snapshot of what `freearc-native` decodes / extracts on real
archives, plus the synthetic round-trip results from the test
suite. Updated whenever the tool gets a new run against a non-trivial
corpus; commits to this file are the canonical "does it actually
work" record.

## synthetic round-trip (CI)

Cargo integration test in `src/writer.rs`:

| test                                           | input                          | result |
|------------------------------------------------|--------------------------------|--------|
| `writer::tests::write_then_open_roundtrips_one_file`   | 1 text file, 16 bytes      | PASS   |
| `writer::tests::write_then_extract_roundtrips_three_files` | text + 2 KB random bin | PASS, 0 CRC mismatch |
| 20 codec/varint/dir unit tests                 | synth fixtures                 | PASS   |

Run on every push: `cargo test --release --lib` from the crate root.

## CLI round-trip, Linux x86_64 (Ubuntu 24.04, kernel 6.17)

```
$ fg-arc-c /tmp/rt-in /tmp/rt.arc
wrote /tmp/rt.arc (3 files, 4117 original bytes)

$ fg-arc-ls /tmp/rt.arc
archive: /tmp/rt.arc (4300 bytes)
footer:  pos=4277 origsize=38 compsize=38 compressor="storing" type=0x04 (FOOTER)
2 control block(s):
  HEADER abs_pos=0    origsize=8   compsize=8   storing
  DIR    abs_pos=4148 origsize=68  compsize=68  storing

$ fg-arc-files /tmp/rt.arc
- 6    363a3020 solid=0  a.txt
- 15   9b6332b0 solid=0  b.txt
- 4096 048b1119 solid=0  c.bin
3 files, 4117 bytes original

$ fg-arc-x /tmp/rt.arc /tmp/rt-out
3 wrote, 0 skipped, 0 CRC mismatch

$ diff -r /tmp/rt-in /tmp/rt-out
(no output -> identical)
```

## CLI round-trip, macOS 15 / Apple Silicon

Same procedure on the Mac via SSH, 3 input files including 8 KB
random binary:

```
wrote /tmp/rt.arc (3 files, 8231 original bytes)
HEADER abs_pos=0 storing 8/8 + DIR abs_pos=8262 storing 67/67
3 wrote, 0 skipped, 0 CRC mismatch
ROUND-TRIP OK ON MAC
```

## real-world survey, ~/Games-source on the Mac

`fg-arc-survey` walked Joe's full game-source directory
(2026-05-28, 698 candidate `.bin` files):

```
scanned:  1 FreeArc archives
NATIVE:   0 archive(s), 0 bytes
HYBRID:   1 archive(s), 9578531 bytes
BLOCKED:  0
BROKEN:   0
        0.81 real
```

The one FreeArc archive is the CoD MW3 FitGirl test bin
(`fg-05.bin`). It's HYBRID because the data block uses
`srep:m3f:l256+dispack070+delta+lollypop:d1024:al1:mc1023:...` and
the lollypop CLS plugin is blocked on wine-on-Mac (see top-level
README "known issues"). The other 697 `.bin` files in the corpus
are non-FreeArc game assets (Need for Speed car geometry, Unity
scene_info, CarX Street data, etc.) — the magic-byte pre-filter
caught them all.

### what this means

For users with FitGirl repacks only: cellar's native path
extracts nothing. Use the `archive_peek` UI to inspect, then go
to a Windows machine for actual extraction.

For users with other FreeArc archive sources (DODI low-press,
KaOs, plain Inno setups, FreeArc-the-archiver users): run
`fg-arc-survey` on the folder and any archive marked NATIVE is
extractable today via `fg-arc-x` without wine.

If you have such a corpus and want it included here, send a PR
adding the survey output.
