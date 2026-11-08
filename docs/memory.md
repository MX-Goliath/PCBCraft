# Editing memory usage — October 21, 2026

## Causes of growth and fixes

1. History stored an unlimited number of complete Documents. In schematics,
   every change cloned the original Altium file in base64, SourceRecord and images.
   Historical source, records and image resources are now shared through Arc;
   resource changes use copy-on-write. The JSON v1 format is unchanged.
2. History is limited to 128 snapshots and 64 MiB of a conservative **estimate of
   editable snapshots per document**. Whichever limit is reached first applies;
   large schematics retain fewer than 128 steps. Old snapshots are removed first;
   revisions remain monotonic. The nearest single oversized snapshot is retained
   even over budget so the latest operation remains undoable. This is not a limit
   on total process RAM: open documents, the single native archive, libraries,
   fonts, GPU and allocator reserves are accounted for separately. Limits can be
   changed through History::with_limits; there is no UI setting for them yet.
3. Image TextureHandles are no longer retained indefinitely in egui temp data:
   the frame cache releases unused textures after closing/hiding/replacing them.
   The same images in adjacent frames reuse their texture IDs.
4. The PCB cache removes unused triangulations after one skipped frame instead
   of retaining up to 256 stale preview/region entries.
5. Font changes reuse free font slots; hidden labels, the sheet font and separate
   pin-number fonts count as references. Copying text/components retains only
   required images; deleting the last object using an image releases its live
   resource while keeping the image available through Undo.
6. Closing a document clears its Properties draft/dialog. The saved snapshot,
   clipboard and open libraries serve independent purposes; data they still need
   is not forcibly removed. Undo/Redo size accounting is cached when switching
   known snapshots and does not serialize the document on every Undo.

## Measurements

Linux x86_64, Rust 1.90.0, debug build. Phase1.SchDoc from TI PMP23595 was loaded
only from a local directory outside Git. The memory_stress probe used the System
allocator to count **live allocated bytes**, plus Linux VmRSS.
The model/history are measured without the GUI; RSS includes malloc reserves.

Before the fix: after 200 hidden toggles on one text object,
1 954 279 943 bytes of live allocations and 2 493 669 376 bytes RSS.
Closing the document released live data, but RSS stayed around 2.49 GB:
the OS metric alone does not prove that live objects are retained.

After the fix, in decimal MB (1 MB = 1 000 000 bytes):

| Scenario | Repetitions | Final live MB | Final RSS MB | Snapshots |
|---|---:|---:|---:|---:|
| Visibility toggle | 600 | 19.65 | 30.92 | 9 |
| Text change | 400 | 19.65 | 30.88 | 9 |
| Font size change | 400 | 19.65 | 31.39 | 9 |
| Undo/Redo | 600 | 10.98 | 19.33 | 1 |
| No-op edit | 600 | 9.87 | 18.66 | 0 |
| Preview without commit | 600 | 9.87 | 19.29 | 0 |
| Insert/delete | 300 pairs | 20.77 | 32.98 | 9 |
| Open/close | 60 | 9.87 | 33.32 | 0 |

In the visibility test, live data is unchanged at steps 50–600; text edits also
plateau after warm-up. After closing, all eight probes leave about 4 KB of live
allocations in the probe process. In the font probe, the table grows from 14 to
16 definitions and then stops growing.

The Xvfb/Mesa GUI was checked separately on a copy of editing-demo: 200 Comment
visibility toggles in Component Properties, Undo/Redo, Save to the application's
JSON, tab close and exit. RSS: 158.89 → 159.16 MB (mostly backend, fonts and GUI,
so these numbers cannot be compared directly with the model probe).
User originals and TI files were not changed.

## Reproduction and regressions

```sh
cargo run --locked -p eda-io --example memory_stress -- /path/to/input.SchDoc 600 visibility
```

Modes: visibility, text, font, undo-redo, noop, preview, insert-delete,
open-close. The probe prints JSONL, does not write the input document and also
works with the application's JSON. RSS is null on platforms without /proc;
the live allocation counter works independently of /proc.

Verified: 122 workspace tests, cargo fmt --all -- --check,
workspace/all-targets Clippy with -D warnings and workspace/all-targets build.
New regressions cover history limits/branching/oversized Undo, Arc sharing and
native asset release, font-slot reuse, clipboard resources, image deletion with
Undo/Redo and texture/triangulation cache release.
Existing older JSON/schematics and libraries continue to pass IO roundtrip tests.

Full reports remain in a local memory-validation-2026-10-21 directory outside Git.
These and the original TI projects are not included in Git. The listed scenarios
were verified on Linux; Windows profiling and GPU VRAM counters are outside this report.
