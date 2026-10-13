//! Safe-area insets per platform: iOS reads them from `UIKit`; Android (`ManhwaStudio` fork) takes
//! the physical-pixel insets the forked winit reports per window
//! (`winit::platform::android::WindowExtAndroid::insets`) and converts them to points.

#[cfg(target_os = "ios")]
pub use ios::get_safe_area_insets;

#[cfg(target_os = "ios")]
mod ios {
    use egui::{SafeAreaInsets, epaint::MarginF32};
    use objc2::{ClassType, rc::Retained};
    use objc2_foundation::{MainThreadMarker, NSObjectProtocol};
    use objc2_ui_kit::{UIApplication, UISceneActivationState, UIWindowScene};

    /// Gets the ios safe area insets.
    ///
    /// A safe area defines the area within a view that isn’t covered by a navigation bar, tab bar,
    /// toolbar, or other views a window might provide. Safe areas are essential for avoiding a
    /// device’s interactive and display features, like Dynamic Island on iPhone or the camera
    /// housing on some Mac models.
    ///
    /// Once winit v0.31 has been released this can be removed in favor of
    /// `winit::Window::safe_area`.
    pub fn get_safe_area_insets() -> SafeAreaInsets {
        let Some(main_thread_marker) = MainThreadMarker::new() else {
            log::error!("Getting safe area insets needs to be performed on the main thread");
            return SafeAreaInsets::default();
        };

        let app = UIApplication::sharedApplication(main_thread_marker);

        #[expect(unsafe_code)]
        unsafe {
            // Look for the first window scene that's in the foreground
            for scene in app.connectedScenes() {
                if scene.isKindOfClass(UIWindowScene::class())
                    && matches!(
                        scene.activationState(),
                        UISceneActivationState::ForegroundActive
                            | UISceneActivationState::ForegroundInactive
                    )
                {
                    // SAFETY: class kind was checked above with `isKindOfClass`
                    let window_scene = Retained::cast_unchecked::<UIWindowScene>(scene.clone());
                    if let Some(window) = window_scene.keyWindow() {
                        let insets = window.safeAreaInsets();
                        return SafeAreaInsets(MarginF32 {
                            top: insets.top as f32,
                            left: insets.left as f32,
                            right: insets.right as f32,
                            bottom: insets.bottom as f32,
                        });
                    }
                }
            }
        }

        SafeAreaInsets::default()
    }
}

/// Converts safe-area insets in physical pixels (`left`, `top`, `right`, `bottom`) to egui points
/// at `pixels_per_point` (native scale factor times egui zoom). A non-finite or non-positive
/// `pixels_per_point` yields zero insets. (`ManhwaStudio` fork, used on Android.)
#[cfg(any(target_os = "android", test))]
pub(crate) fn safe_area_insets_from_physical(
    [left, top, right, bottom]: [u32; 4],
    pixels_per_point: f32,
) -> egui::SafeAreaInsets {
    if !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
        return egui::SafeAreaInsets::default();
    }
    // Exact conversion: real insets are a few hundred pixels; anything past u16 is clamped.
    let points = |px: u32| f32::from(u16::try_from(px).unwrap_or(u16::MAX)) / pixels_per_point;
    egui::SafeAreaInsets(egui::epaint::MarginF32 {
        left: points(left),
        top: points(top),
        right: points(right),
        bottom: points(bottom),
    })
}

#[cfg(test)]
mod tests {
    use super::safe_area_insets_from_physical;

    #[test]
    fn physical_insets_become_points_at_the_current_scale() {
        let insets = safe_area_insets_from_physical([0, 72, 30, 48], 3.0);
        assert_eq!(insets.0.left, 0.0);
        assert_eq!(insets.0.top, 24.0);
        assert_eq!(insets.0.right, 10.0);
        assert_eq!(insets.0.bottom, 16.0);
        // A UI zoom of 1.5 on top of a 2.0 density: 3.0 pixels per point as well.
        assert_eq!(safe_area_insets_from_physical([0, 72, 30, 48], 2.0 * 1.5), insets);
    }

    #[test]
    fn invalid_scale_gives_zero_insets() {
        for ppp in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(safe_area_insets_from_physical([10, 20, 30, 40], ppp), Default::default());
        }
    }
}
