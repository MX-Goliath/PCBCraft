# PCBCraft

An independent EDA application in Rust for Windows and Linux. Altium Designer is
an observation reference, not the execution engine. The existing
`ALTIUM_RUST_AGENT_PLAN.md` preserves the full target scope.

The user-defined visual target is an exact copy of the available Altium Designer
Enterprise 24.4.1 interface. Do not replace its layout with a custom design.
Compare dimensions, menu order, panels, tabs, icons, colors, typography and states
at the same window size and DPI. Keep the same layout on Linux; explicitly account
for platform differences and unverified matches. PLAN.md is the working log of
the original plan, not its replacement.

- The model and commands belong in `eda-core`, with no GUI/GPU dependencies.
- Screen coordinates belong to `eda-render`; document coordinates belong to `eda-geom`.
- The renderer reads the model. Changes go through validation and history.
- Do not declare a feature complete just because it has a button. Disable
  unimplemented commands; record differences in `docs/features.json`.
- Observation, assumption, implementation and verification are separate statuses.
- Do not modify the user's Altium project for research unless necessary.
- Before completing a stage: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Check the real application window after significant interface changes.
- Do not add empty crates merely to match an architecture diagram.
- Do not port KiCad code or its execution engine.
- Keep only one PCBCraft GUI instance open. Close the previous test instance
  before launching another build.
