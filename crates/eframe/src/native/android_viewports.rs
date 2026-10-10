//! Android multi-window rules of the glow integration (`ManhwaStudio` fork).
//!
//! With the forked winit, every viewport window after the main one may be a separate OS window
//! (a `SecondaryWindowActivity`, see `winit::platform::android::multiwindow`). Such a window is
//! created without a native surface; its surface arrives with `WindowEvent::Resized` /
//! `WindowEvent::Occluded(false)`, goes away with `Occluded(true)` (winit waits for the GL surface
//! to be dropped before acknowledging Android's `surfaceDestroyed`), and a replacement surface
//! bumps the window's surface generation. The main window keeps the stock lifecycle: its surface
//! lives from `Resumed` to `Suspended`.
//!
//! This file holds the pure decision ([`surface_action`]) so it is unit-tested on the host; the
//! glow integration performs it. Compiled for Android and for host tests only.

/// What the window reports about its native surface, and what the viewport holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SurfaceFacts {
    /// The window is the main (activity) window or a handle to it.
    pub is_main_window: bool,

    /// The window has a native surface now (`WindowExtAndroid::has_native_surface`).
    pub has_native_surface: bool,

    /// The window's current surface generation (`WindowExtAndroid::surface_generation`).
    pub surface_generation: u64,

    /// The generation the viewport's GL surface was built for, `None` without a GL surface.
    pub gl_surface_generation: Option<u64>,
}

/// What to do with a viewport's GL surface (see [`surface_action`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceAction {
    /// The GL surface is valid: nothing to do.
    Keep,

    /// No GL surface yet and the window can take one: create it.
    Create,

    /// The GL surface was built on an older native surface: release it, then create a new one.
    Recreate,

    /// The window lost its native surface: release the GL surface and create none.
    Release,

    /// No GL surface and no native surface: wait for the surface (UI runs, nothing is painted).
    WaitForSurface,
}

impl SurfaceAction {
    /// Whether the current GL surface must be released first.
    pub(crate) fn releases(self) -> bool {
        matches!(self, Self::Recreate | Self::Release)
    }

    /// Whether a GL surface is to be created (after any release).
    pub(crate) fn creates(self) -> bool {
        matches!(self, Self::Create | Self::Recreate)
    }
}

/// Decides what a viewport's GL surface needs, given its window's surface state.
///
/// The main window keeps the stock behaviour exactly: a missing GL surface is created (its window
/// only exists between `Resumed` and `Suspended`) and an existing one is kept (the main window's
/// generation is always 0). A secondary window follows its native surface: none -> no GL surface,
/// a new generation -> a new GL surface.
pub(crate) fn surface_action(facts: SurfaceFacts) -> SurfaceAction {
    let SurfaceFacts {
        is_main_window,
        has_native_surface,
        surface_generation,
        gl_surface_generation,
    } = facts;

    if is_main_window {
        return match gl_surface_generation {
            Some(_) => SurfaceAction::Keep,
            None => SurfaceAction::Create,
        };
    }

    match (has_native_surface, gl_surface_generation) {
        (false, Some(_)) => SurfaceAction::Release,
        (false, None) => SurfaceAction::WaitForSurface,
        (true, None) => SurfaceAction::Create,
        (true, Some(built)) if built == surface_generation => SurfaceAction::Keep,
        (true, Some(_)) => SurfaceAction::Recreate,
    }
}

#[cfg(test)]
mod tests {
    use super::{SurfaceAction, SurfaceFacts, surface_action};

    fn secondary(has_native_surface: bool, generation: u64, built: Option<u64>) -> SurfaceFacts {
        SurfaceFacts {
            is_main_window: false,
            has_native_surface,
            surface_generation: generation,
            gl_surface_generation: built,
        }
    }

    #[test]
    fn main_window_keeps_stock_behaviour() {
        let main = |has_native_surface, built| SurfaceFacts {
            is_main_window: true,
            has_native_surface,
            surface_generation: 0,
            gl_surface_generation: built,
        };
        assert_eq!(surface_action(main(true, None)), SurfaceAction::Create);
        assert_eq!(surface_action(main(true, Some(0))), SurfaceAction::Keep);
        // Even a (theoretical) surface-less main window is treated as today: create on demand,
        // keep what exists. Its surface lifecycle is `Resumed` / `Suspended`, not this rule.
        assert_eq!(surface_action(main(false, None)), SurfaceAction::Create);
        assert_eq!(surface_action(main(false, Some(0))), SurfaceAction::Keep);
    }

    #[test]
    fn secondary_window_follows_its_surface_lifecycle() {
        // Created without a surface.
        let fresh = surface_action(secondary(false, 0, None));
        assert_eq!(fresh, SurfaceAction::WaitForSurface);
        assert!(!fresh.creates() && !fresh.releases());

        // The surface arrives (`Resized` / `Occluded(false)`).
        let arrived = surface_action(secondary(true, 1, None));
        assert_eq!(arrived, SurfaceAction::Create);
        assert!(arrived.creates() && !arrived.releases());

        // Painting with the surface it was built for.
        assert_eq!(surface_action(secondary(true, 1, Some(1))), SurfaceAction::Keep);

        // `Occluded(true)`: winit already reports no surface while it still holds the window.
        let gone = surface_action(secondary(false, 1, Some(1)));
        assert_eq!(gone, SurfaceAction::Release);
        assert!(gone.releases() && !gone.creates());

        // A replacement surface without a destroy in between.
        let replaced = surface_action(secondary(true, 2, Some(1)));
        assert_eq!(replaced, SurfaceAction::Recreate);
        assert!(replaced.releases() && replaced.creates());
    }
}
