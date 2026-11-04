# Repository privacy

Only source code, public references, reviewed synthetic fixtures and anonymized
verification results belong in Git. Keep private designs in `private-projects/`
or outside the checkout. Local captures, reports and application state are ignored.
Do not force-add these files. Ignore rules do not remove files already tracked.

ReferenceA, ReferenceB and similar labels in observation logs are anonymized
private reference cases, not distributable sample projects. Original filenames,
personal paths and source-file fingerprints are intentionally omitted. Historical
measurements and limitations remain observations, not claims of full compatibility.
Third-party font licenses and public documentation links retain their attribution.

Before committing, run `python scripts/check_privacy.py` and review the staged
diff, especially JSON, binary assets and any new fixture. CI runs the same check.
Enable the included pre-commit hook with `git config core.hooksPath .githooks`.
Use a neutral repository-local Git author if you do not want personal commit
metadata, for example `MX-Goliath <contributors@example.invalid>`.
Use `python scripts/check_privacy.py --history` to also inspect all reachable refs,
including author/committer metadata. The check catches common home paths, emails,
credential formats and misplaced design files; private project names, drawings,
encoded data and images still require human review. Checks inspect both staged
content and working-tree versions of tracked files, plus history when requested.

Private verification reports default to `target/pcb-update-checks/`. They can
contain full paths, project metadata and original-file hashes; do not publish
them directly. Commit only reviewed summaries with anonymous case labels.

After a history cleanup, existing clones must be replaced or reset to the cleaned
history before publishing further changes. Merging an old branch can reintroduce
removed content. Remote caches, forks, other clones, build logs and downloaded
artifacts are separate from reachable Git history and need separate handling.
