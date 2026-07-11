#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Viewport {
    pub columns: u16,
    pub rows: u16,
    pub pixel_width: Option<u32>,
    pub pixel_height: Option<u32>,
}

impl Viewport {
    pub const fn cells(columns: u16, rows: u16) -> Self {
        Self {
            columns,
            rows,
            pixel_width: None,
            pixel_height: None,
        }
    }

    pub const fn with_pixels(columns: u16, rows: u16, pixel_width: u32, pixel_height: u32) -> Self {
        Self {
            columns,
            rows,
            pixel_width: Some(pixel_width),
            pixel_height: Some(pixel_height),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum LayoutSensitivity {
    #[default]
    Independent,
    Columns,
    Pixels,
    FullViewport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResizeAction {
    Reuse,
    Rerender,
}

pub fn resize_action(
    previous: Viewport,
    next: Viewport,
    sensitivity: LayoutSensitivity,
) -> ResizeAction {
    let changed = match sensitivity {
        LayoutSensitivity::Independent => false,
        LayoutSensitivity::Columns => previous.columns != next.columns,
        LayoutSensitivity::Pixels => match (
            previous.pixel_width,
            previous.pixel_height,
            next.pixel_width,
            next.pixel_height,
        ) {
            (Some(old_width), Some(old_height), Some(new_width), Some(new_height)) => {
                old_width != new_width || old_height != new_height
            }
            _ => previous.columns != next.columns || previous.rows != next.rows,
        },
        LayoutSensitivity::FullViewport => previous != next,
    };

    if changed {
        ResizeAction::Rerender
    } else {
        ResizeAction::Reuse
    }
}

pub fn stable_fingerprint(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001b3;

    bytes.iter().fold(OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::{LayoutSensitivity, ResizeAction, Viewport, resize_action};

    #[test]
    fn column_layout_ignores_height_only_changes() {
        assert_eq!(
            resize_action(
                Viewport::cells(80, 24),
                Viewport::cells(80, 50),
                LayoutSensitivity::Columns,
            ),
            ResizeAction::Reuse
        );
        assert_eq!(
            resize_action(
                Viewport::cells(80, 24),
                Viewport::cells(120, 24),
                LayoutSensitivity::Columns,
            ),
            ResizeAction::Rerender
        );
    }

    #[test]
    fn pixel_layout_tracks_pixel_geometry() {
        assert_eq!(
            resize_action(
                Viewport::with_pixels(80, 24, 800, 480),
                Viewport::with_pixels(80, 24, 1000, 480),
                LayoutSensitivity::Pixels,
            ),
            ResizeAction::Rerender
        );
    }
}
