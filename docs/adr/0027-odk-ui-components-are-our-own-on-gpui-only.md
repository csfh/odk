# ODK's UI components are our own, built on GPUI only, shipped as a versioned crate

ODK provides a stock set of UI components for applications: buttons, inputs, dialogs and the like. We write them ourselves on top of GPUI and depend on no third-party component library. An existing one, `gpui-component`, is on crates.io and describes itself as based on shadcn/ui; we rejected it so that GPUI is the only moving, pre-1.0 UI dependency we track (ADR 0004) and the components follow Omarchy's theme the way ADR 0019 requires, under our control.

"shadcn-like" here means the quality and the API, not the distribution. Components are small, composable pieces with consistent naming for variants and sizes, sensible accessible defaults, and colours that come from theme tokens rather than being written into the component. We rejected shadcn's copy-in model, where a tool copies component source into each application: it would put GPUI types directly into application code against ADR 0004's wrapper, make every application follow GPUI's breaking changes alone, and need a copy tool and a registry besides. Applications depend on the crate and get fixes by updating it.

## Consequences

The crate layout, the GPUI pinning mechanism (a crates.io release or a git commit; `gpui` is on crates.io, version 0.2.2 when checked, and ADR 0004 says "commit"), the token set and the component list are separate decisions still to be made.
