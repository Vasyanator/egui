# Module: crates/eframe/src/native (ManhwaStudio fork notes)

## Purpose
eframe's native backend: the winit event loop (`run.rs`), the `WinitApp` contract
(`winit_integration.rs`) and its glow / wgpu implementations. This file documents only what the
ManhwaStudio fork (branch `manhwastudio-android`) adds; everything else is upstream eframe.

## Android multi-window (glow only)
The forked winit (`winit::platform::android::multiwindow`) can back every window after the main
one by its own `SecondaryWindowActivity` (a free-form OS window on DeX / pop-up view). The glow
integration follows that model; desktop code paths are unchanged.

- **Embed gate** (`winit_integration.rs::native_viewports_supported`): extra viewports are real
  windows on desktop, and on Android when `multiwindow::native_windows_available()` is true. It is
  read by `create_egui_context`, i.e. once per `run_native` (after the event loop exists); an app
  sets the policy (`multiwindow::set_native_window_policy`) before `run_native`. A context passed
  to `run_native_ext` keeps its own setting.
- **Embed fallback**: `glow_integration.rs::poll_android_embed_fallback` takes
  `multiwindow::take_embed_fallback_request()` on every window event; when a secondary window came
  up full-screen, the session switches to embedded viewports (`set_embed_viewports(true)`), and
  the native windows still open are dropped by the next root frame.
- **One GL context, one EGL surface per window.** A secondary window starts without a native
  surface: its viewport gets the winit window and `egui_winit::State` at once, its GL surface only
  when the window reports a surface (`Resized` / `Occluded(false)`). The surface is released on
  `Occluded(true)` before that event returns (winit then releases the `ANativeWindow` and
  acknowledges Android's `surfaceDestroyed`, which blocks the UI thread up to 2 s), and re-created
  when the window's surface generation changes. The decision is the pure
  `android_viewports.rs::surface_action` (host-tested); `initialize_window` performs it.
- **No surface, no painting**: such a viewport runs its UI (immediate) or is treated as minimized
  (deferred); platform output and nested viewports are still handled.
- **Releasing a surface** (`release_gl_surface`) first makes the context not current when it is
  current on that surface, so EGL does not defer the destroy and keep the window connected.
- **`Suspended` is about the main Activity**: `on_suspend` drops window + surface only for
  viewports on the main window; secondary windows keep theirs. The context is made not current
  before the surfaces are dropped.
- **Exit**: `save_and_destroy` tolerates a missing root window (a run that ends while suspended)
  and, on Android, makes the context not current and drops every surface before the run state is
  dropped, so the next `run_native` can create a surface on the same `ANativeWindow`.

## Known gaps
- Immediate secondary windows are painted from their parent's (root's) frame: while the main
  Activity has no surface (backgrounded, root window dropped) they do not repaint.
- No GL code is unit-testable on the host; only `android_viewports.rs` has tests. The lifecycle
  itself is verified on a device (see `dev-docs/ANDROID_PORT.md` in ManhwaStudio).

## Editing map
- Surface lifecycle rule: `android_viewports.rs` (+ test), then `glow_integration.rs`
  (`initialize_window`, `update_android_surface`, `on_suspend`, `release_surfaces_for_exit`).
- Embed decision: `winit_integration.rs::native_viewports_supported`.
- The wgpu integration has none of this (ManhwaStudio uses glow).
