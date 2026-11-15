use crate::models::{CardID, FakeNotchStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Workspace,
    Week,
    Insights,
    Settings,
}

impl Tab {
    pub fn from_cli(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "workspace" => Some(Tab::Workspace),
            "week" => Some(Tab::Week),
            "insights" => Some(Tab::Insights),
            "settings" => Some(Tab::Settings),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }

    pub fn inflated(&self, dx: f64, dy: f64) -> Self {
        Self {
            x: self.x - dx,
            y: self.y - dy,
            w: self.w + dx * 2.0,
            h: self.h + dy * 2.0,
        }
    }

    pub fn union(&self, other: Self) -> Self {
        let min_x = self.x.min(other.x);
        let min_y = self.y.min(other.y);
        let max_x = (self.x + self.w).max(other.x + other.w);
        let max_y = (self.y + self.h).max(other.y + other.h);
        Self {
            x: min_x,
            y: min_y,
            w: max_x - min_x,
            h: max_y - min_y,
        }
    }
}

/// Logical geometry matching macOS `NotchGeometry.swift` and `NotchController.swift`.
pub struct IslandGeometry;

impl IslandGeometry {
    pub const ISLAND_W: f64 = 116.0;
    pub const ISLAND_H: f64 = 22.0;
    pub const ISLAND_TOP_MARGIN: f64 = 3.0;
    pub const PILL_W: f64 = 76.0;
    pub const PILL_H: f64 = 22.0;
    pub const PILL_GAP: f64 = 8.0;
    pub const CHIP_W: f64 = 46.0;
    pub const CHIP_H: f64 = 22.0;
    pub const CHIP_GAP: f64 = 8.0;
    pub const COMPACT_W: f64 = 440.0;
    pub const COMPACT_H: f64 = 68.0;
    pub const PANEL_H: f64 = 352.0;
    pub const MODAL_EXTRA_H: f64 = 36.0;
    pub const OVERLAY_MAX_W: f64 = 1340.0;
    pub const OVERLAY_H: f64 = 430.0;

    pub fn desired_panel_width(tab: Tab, visible_cards: &[CardID], screen_w: f64) -> f64 {
        let raw = match tab {
            Tab::Workspace => {
                if visible_cards.is_empty() {
                    830.0
                } else {
                    let sum: f64 = visible_cards.iter().map(|c| c.preferred_width()).sum();
                    let gaps = ((visible_cards.len().saturating_sub(1)) as f64) * 10.0;
                    sum + gaps + 24.0
                }
            }
            Tab::Week => 840.0,
            Tab::Insights | Tab::Settings => 780.0,
        };
        raw.min((screen_w - 24.0).max(520.0))
    }

    pub fn overlay_window_rect(screen_w: f64) -> Rect {
        let w = Self::OVERLAY_MAX_W.min(screen_w);
        let x = ((screen_w - w) * 0.5).floor();
        Rect::new(x, 0.0, w, Self::OVERLAY_H)
    }

    pub fn shape_rect(screen_w: f64, style: FakeNotchStyle) -> Option<Rect> {
        if style == FakeNotchStyle::Off {
            return None;
        }
        let y = if style == FakeNotchStyle::Island {
            Self::ISLAND_TOP_MARGIN
        } else {
            0.0
        };
        let x = (screen_w - Self::ISLAND_W) * 0.5;
        Some(Rect::new(x, y, Self::ISLAND_W, Self::ISLAND_H))
    }

    pub fn pill_rect(screen_w: f64, style: FakeNotchStyle) -> Rect {
        let y = if style == FakeNotchStyle::Notch {
            0.0
        } else {
            Self::ISLAND_TOP_MARGIN
        };
        let island_right = (screen_w + Self::ISLAND_W) * 0.5;
        let x = match style {
            FakeNotchStyle::Off => (screen_w - Self::PILL_W) * 0.5,
            FakeNotchStyle::Island | FakeNotchStyle::Notch => island_right + Self::PILL_GAP,
        };
        Rect::new(x, y, Self::PILL_W, Self::PILL_H)
    }

    pub fn chip_rect(screen_w: f64, style: FakeNotchStyle, has_pill: bool) -> Rect {
        let y = if style == FakeNotchStyle::Notch {
            0.0
        } else {
            Self::ISLAND_TOP_MARGIN
        };
        let island_left = (screen_w - Self::ISLAND_W) * 0.5;
        let x = match style {
            FakeNotchStyle::Off => {
                if has_pill {
                    (screen_w - Self::PILL_W) * 0.5 - Self::CHIP_GAP - Self::CHIP_W
                } else {
                    (screen_w - Self::CHIP_W) * 0.5
                }
            }
            FakeNotchStyle::Island | FakeNotchStyle::Notch => {
                island_left - Self::CHIP_GAP - Self::CHIP_W
            }
        };
        Rect::new(x, y, Self::CHIP_W, Self::CHIP_H)
    }

    pub fn collapsed_hit_rect(
        screen_w: f64,
        style: FakeNotchStyle,
        show_pill: bool,
        show_chip: bool,
        compact_active: bool,
    ) -> Option<Rect> {
        if compact_active {
            let y = if style == FakeNotchStyle::Notch {
                0.0
            } else {
                Self::ISLAND_TOP_MARGIN
            };
            let w = Self::COMPACT_W.min(screen_w - 24.0);
            let x = (screen_w - w) * 0.5;
            return Some(Rect::new(x, y, w, Self::COMPACT_H));
        }

        let mut acc: Option<Rect> = Self::shape_rect(screen_w, style);
        let merge = |cur: Option<Rect>, next: Rect| -> Option<Rect> {
            Some(match cur {
                Some(c) => c.union(next),
                None => next,
            })
        };
        if show_pill {
            acc = merge(acc, Self::pill_rect(screen_w, style));
        }
        if show_chip {
            acc = merge(acc, Self::chip_rect(screen_w, style, show_pill));
        }
        acc.map(|r| Rect::new(r.x - 4.0, 0.0, r.w + 8.0, r.y + r.h + 4.0))
    }

    pub fn expanded_rect(
        screen_w: f64,
        _style: FakeNotchStyle,
        tab: Tab,
        visible_cards: &[CardID],
        has_banner_or_modal: bool,
    ) -> Rect {
        let w = Self::desired_panel_width(tab, visible_cards, screen_w);
        let h = Self::PANEL_H + if has_banner_or_modal { Self::MODAL_EXTRA_H } else { 0.0 };
        let y = 0.0;
        let x = (screen_w - w) * 0.5;
        Rect::new(x, y, w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_widths_match_macos_layout_contract() {
        let cards = vec![CardID::Tasks, CardID::Timer, CardID::Notepad];
        // 330 + 236 + 220 + 20 + 24 = 830
        assert_eq!(
            IslandGeometry::desired_panel_width(Tab::Workspace, &cards, 1920.0),
            830.0
        );
        assert_eq!(
            IslandGeometry::desired_panel_width(Tab::Week, &cards, 1920.0),
            840.0
        );
        assert_eq!(
            IslandGeometry::desired_panel_width(Tab::Insights, &cards, 1920.0),
            780.0
        );
    }

    #[test]
    fn collapsed_hit_rect_combines_island_pill_and_chip() {
        let hit = IslandGeometry::collapsed_hit_rect(1920.0, FakeNotchStyle::Island, true, true, false)
            .expect("collapsed hit rect");
        // Left side has chip, middle has island, right has pill
        assert!(hit.contains(960.0, 10.0)); // center of island
        assert!(hit.contains(960.0 + 70.0, 10.0)); // pill
        assert!(hit.contains(960.0 - 70.0, 10.0)); // chip
        assert!(!hit.contains(960.0, 60.0));
    }
}
