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

## Android safe area (egui-winit + egui, all platforms' code paths)
Android draws edge-to-edge, so system bars, the display cutout and the on-screen keyboard overlap
every window. The fork feeds the forked winit's per-window insets into egui's existing
`RawInput::safe_area_insets` and makes egui honour them; the two changes outside eframe are marked
"`ManhwaStudio` fork" in their comments:

- **egui-winit** `State::take_egui_input` (Android only, `set_android_safe_area`): sets
  `safe_area_insets` every frame from `WindowExtAndroid::insets().safe_area` (physical px),
  converted to points at the current `pixels_per_point` (`safe_area.rs::
  safe_area_insets_from_physical`, host-tested), and logs each change (`egui-winit insets:`).
- **egui** `Context::run_ui`: the root `Ui` is `content_rect()` (viewport minus safe area) instead
  of `viewport_rect()`, so `Panel` / `CentralPanel` stay in the safe area like `Area`s already did.
  With zero insets the two rects are equal, so desktop layout is unchanged (test
  `context::test::root_ui_is_the_content_rect`).
- **egui** `ContextImpl::begin_pass`: the zoom-change `screen_rect` rescale starts from
  `viewport_rect()` (upstream used `content_rect()`, subtracting the insets twice for one frame).
- The inset bands themselves show the clear colour; full-viewport scrims keep using
  `viewport_rect()`.
- **egui** `TextEdit` (Android only, `cfg!(target_os = "android")`): the safe area includes the
  soft keyboard, so opening it can clip the focused field out of the shrunken content rect.
  Upstream requests IME output only for a VISIBLE field, which then dropped the IME, hid the
  keyboard, restored the layout and re-enabled it: a show/hide loop every frame. The fork keeps
  requesting IME output for a focused, editable field while it is clipped, and scrolls its cursor
  into view when the content rect changes (`android_content_rect_changed`, temp data per field).
  The winit fork additionally delays a hide by a grace period (`platform_impl/android/ime.rs`).

## Known gaps
- Immediate secondary windows are painted from their parent's (root's) frame: while the main
  Activity has no surface (backgrounded, root window dropped) they do not repaint.
- No GL code is unit-testable on the host; only `android_viewports.rs` has tests. The lifecycle
  itself is verified on a device (see `dev-docs/ANDROID_PORT.md` in ManhwaStudio).

## Editing map
- Surface lifecycle rule: `android_viewports.rs` (+ test), then `glow_integration.rs`
  (`initialize_window`, `update_android_surface`, `on_suspend`, `release_surfaces_for_exit`).
- Embed decision: `winit_integration.rs::native_viewports_supported`.
- Focused field vs. soft keyboard: `crates/egui/src/widgets/text_edit/builder.rs` (the
  `else if cfg!(target_os = "android")` branch after `is_rect_visible`, `set_ime_output`).
- Android safe area: `crates/egui-winit/src/lib.rs` (`set_android_safe_area`),
  `crates/egui-winit/src/safe_area.rs`, `crates/egui/src/context.rs` (`run_ui_dyn`, `begin_pass`).
- The wgpu integration has none of this (ManhwaStudio uses glow).
