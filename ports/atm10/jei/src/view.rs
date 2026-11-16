//! The recipe screen JEI opens over a container on "R" or "U": a tab per category that has
//! recipes for the item, the recipes of the selected tab in pages, the category's catalysts
//! in a column at its left, and a "+" beside each recipe. This is its state and its geometry,
//! without the game; `lib.rs` draws it and feeds it clicks.
use crate::index::{Found, Mode};
use crate::plugin::Display;

/// `[x, y, width, height]` in GUI pixels.
pub type Rect = [f32; 4];

pub fn inside(r: Rect, at: [f32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

/// What the screen shows: the question asked, its answer, and where the player is in it.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeView {
    /// The item asked about; None when a whole category is shown.
    pub item: Option<String>,
    pub mode: Mode,
    pub found: Found,
    /// Index into `found`.
    pub tab: usize,
    pub page: usize,
}

impl RecipeView {
    /// None when there is nothing to show (JEI then stays where it is).
    pub fn new(item: Option<String>, mode: Mode, found: Found) -> Option<Self> {
        (!found.is_empty()).then_some(Self {
            item,
            mode,
            found,
            tab: 0,
            page: 0,
        })
    }
    /// The selected category's index in the recipe index.
    pub fn category(&self) -> usize {
        self.found[self.tab].0
    }
    fn recipes(&self) -> &[usize] {
        &self.found[self.tab].1
    }
    pub fn pages(&self, per_page: usize) -> usize {
        self.recipes().len().div_ceil(per_page.max(1))
    }
    /// The recipes on the page shown, as indices into the category's recipes.
    pub fn on_page(&self, per_page: usize) -> &[usize] {
        let per_page = per_page.max(1);
        let start = (self.page * per_page).min(self.recipes().len());
        let end = (start + per_page).min(self.recipes().len());
        &self.recipes()[start..end]
    }
    /// Turn the page; both ends wrap round, as JEI's arrows do.
    pub fn turn_page(&mut self, forward: bool, per_page: usize) {
        let pages = self.pages(per_page).max(1);
        self.page = if forward {
            (self.page + 1) % pages
        } else {
            (self.page + pages - 1) % pages
        };
    }
    /// Pick another category's tab, at its first page.
    pub fn select(&mut self, tab: usize) {
        if tab < self.found.len() {
            self.tab = tab;
            self.page = 0;
        }
    }
    pub fn turn_tab(&mut self, forward: bool) {
        let n = self.found.len();
        self.select(if forward {
            (self.tab + 1) % n
        } else {
            (self.tab + n - 1) % n
        });
    }
}

/// The parts of the screen a click can land on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Tab(usize),
    PrevCategory,
    NextCategory,
    PrevPage,
    NextPage,
    /// A slot of the `n`th recipe on the page.
    Slot {
        recipe: usize,
        slot: usize,
    },
    /// The "+" of the `n`th recipe on the page.
    Plus(usize),
    /// A catalyst of the category.
    Catalyst(usize),
    /// Elsewhere on the screen's frame: swallowed.
    Frame,
    /// Outside the screen.
    Outside,
}

const PAD: f32 = 8.0;
const TAB: f32 = 24.0;
const HEADER: f32 = 34.0;
const BUTTON: f32 = 13.0;
const SLOT: f32 = 18.0;
const GAP: f32 = 6.0;

/// Where everything is for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub frame: Rect,
    /// Tabs shown, with the index into `found` each is.
    pub tabs: Vec<(usize, Rect)>,
    pub prev_category: Rect,
    pub next_category: Rect,
    pub prev_page: Rect,
    pub next_page: Rect,
    /// Centre of the title line and of the page line.
    pub title: [f32; 2],
    pub page_label: [f32; 2],
    /// Top left of each recipe on the page, and its "+".
    pub recipes: Vec<([f32; 2], Rect)>,
    pub catalysts: Vec<Rect>,
    pub per_page: usize,
}

/// How the screen sits on a `screen`-sized window for a category `size`d recipe.
pub fn layout(screen: [f32; 2], size: [u16; 2], view: &RecipeView, catalysts: usize) -> Geometry {
    let (cw, ch) = (size[0] as f32, size[1] as f32);
    let width = (cw + 2.0 * PAD + BUTTON + 4.0).max(176.0);
    let max_height = (screen[1] - TAB - 2.0 * PAD).max(ch + HEADER + PAD);
    let per_page = (((max_height - HEADER - PAD) / (ch + GAP)).floor() as usize).max(1);
    let shown = view.on_page(per_page).len().max(1);
    let height = HEADER + PAD + shown as f32 * (ch + GAP);
    let x = ((screen[0] - width) / 2.0).floor();
    let y = (((screen[1] - height) / 2.0).floor()).max(TAB);
    let frame = [x, y, width, height];

    let visible = ((width / TAB).floor() as usize).max(1);
    let first = view
        .tab
        .saturating_sub(visible - 1)
        .min(view.found.len().saturating_sub(visible));
    let tabs = (first..view.found.len().min(first + visible))
        .enumerate()
        .map(|(i, tab)| {
            (
                tab,
                [x + i as f32 * TAB, y - TAB + 2.0, TAB - 2.0, TAB - 2.0],
            )
        })
        .collect();

    let left = x + (width - cw - BUTTON - 4.0) / 2.0;
    let recipes = (0..shown)
        .map(|i| {
            let top = y + HEADER + i as f32 * (ch + GAP);
            (
                [left, top],
                [left + cw + 4.0, top + ch - BUTTON, BUTTON, BUTTON],
            )
        })
        .collect();
    let catalysts = (0..catalysts)
        .map(|i| [x - SLOT - 6.0, y + 4.0 + i as f32 * SLOT, SLOT, SLOT])
        .collect();
    Geometry {
        frame,
        tabs,
        prev_category: [x + 4.0, y + 4.0, BUTTON, BUTTON],
        next_category: [x + width - 4.0 - BUTTON, y + 4.0, BUTTON, BUTTON],
        prev_page: [x + 4.0, y + 19.0, BUTTON, BUTTON],
        next_page: [x + width - 4.0 - BUTTON, y + 19.0, BUTTON, BUTTON],
        title: [x + width / 2.0, y + 7.0],
        page_label: [x + width / 2.0, y + 22.0],
        recipes,
        catalysts,
        per_page,
    }
}

impl Geometry {
    /// The 18×18 slot of a recipe slot whose item sits at `pos` inside the recipe at `origin`.
    pub fn slot_rect(origin: [f32; 2], pos: [i16; 2]) -> Rect {
        [
            origin[0] + pos[0] as f32 - 1.0,
            origin[1] + pos[1] as f32 - 1.0,
            SLOT,
            SLOT,
        ]
    }

    /// What lies under `at`, given the recipes on the page in order.
    pub fn hit(&self, at: [f32; 2], page: &[&Display]) -> Hit {
        if let Some((tab, _)) = self.tabs.iter().find(|(_, r)| inside(*r, at)) {
            return Hit::Tab(*tab);
        }
        if let Some(i) = self.catalysts.iter().position(|r| inside(*r, at)) {
            return Hit::Catalyst(i);
        }
        for (rect, hit) in [
            (self.prev_category, Hit::PrevCategory),
            (self.next_category, Hit::NextCategory),
            (self.prev_page, Hit::PrevPage),
            (self.next_page, Hit::NextPage),
        ] {
            if inside(rect, at) {
                return hit;
            }
        }
        for (n, ((origin, plus), display)) in self.recipes.iter().zip(page).enumerate() {
            if display.recipe_id.is_some() && inside(*plus, at) {
                return Hit::Plus(n);
            }
            if let Some(slot) = display
                .slots
                .iter()
                .position(|s| inside(Self::slot_rect(*origin, s.pos), at))
            {
                return Hit::Slot { recipe: n, slot };
            }
        }
        if inside(self.frame, at) {
            Hit::Frame
        } else {
            Hit::Outside
        }
    }
}

/// The bookmark list's slots, left of the container's window `window` on a `screen`-sized
/// window, filling columns of 18 px from the top, as many as fit; the list takes the left
/// gap's room, JEI's bookmark overlay.
pub fn bookmark_slots(screen: [f32; 2], window: Rect, count: usize) -> Vec<Rect> {
    let room = window[0] - 2.0 * PAD;
    let columns = ((room / SLOT).floor() as usize).min(9);
    let rows = (((screen[1] - 2.0 * PAD) / SLOT).floor() as usize).max(1);
    if columns == 0 {
        return Vec::new();
    }
    (0..count.min(columns * rows))
        .map(|i| {
            let (row, column) = (i / columns, i % columns);
            [
                PAD + column as f32 * SLOT,
                PAD + row as f32 * SLOT,
                SLOT,
                SLOT,
            ]
        })
        .collect()
}

/// The rectangle around `slots`, for `screen_exclusion_areas`.
pub fn bounds(slots: &[Rect]) -> Option<Rect> {
    let first = slots.first()?;
    let (mut x0, mut y0, mut x1, mut y1) =
        (first[0], first[1], first[0] + first[2], first[1] + first[3]);
    for r in slots {
        x0 = x0.min(r[0]);
        y0 = y0.min(r[1]);
        x1 = x1.max(r[0] + r[2]);
        y1 = y1.max(r[1] + r[3]);
    }
    Some([x0, y0, x1 - x0, y1 - y0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Role;

    fn view(recipes: usize) -> RecipeView {
        RecipeView::new(
            Some("minecraft:stick".into()),
            Mode::Recipes,
            vec![(0, (0..recipes).collect()), (3, vec![0])],
        )
        .unwrap()
    }

    #[test]
    fn pages_wrap_round_both_ways() {
        let mut v = view(5);
        assert_eq!(v.pages(2), 3);
        v.turn_page(false, 2);
        assert_eq!(v.page, 2);
        assert_eq!(v.on_page(2), [4]);
        v.turn_page(true, 2);
        assert_eq!((v.page, v.on_page(2)), (0, &[0, 1][..]));
    }

    #[test]
    fn a_new_tab_starts_at_its_first_page_and_tabs_wrap() {
        let mut v = view(5);
        v.turn_page(true, 2);
        v.turn_tab(true);
        assert_eq!((v.category(), v.page), (3, 0));
        v.turn_tab(true);
        assert_eq!(v.category(), 0);
        assert!(RecipeView::new(None, Mode::Uses, Vec::new()).is_none());
    }

    #[test]
    fn the_screen_fits_as_many_recipes_as_its_height_allows() {
        // A crafting recipe is 54 high; on a 480×270 window the screen leaves a tab row.
        let g = layout([480.0, 270.0], [116, 54], &view(9), 1);
        assert_eq!(g.per_page, 3);
        assert_eq!(g.recipes.len(), 3);
        assert!(g.frame[1] + g.frame[3] <= 270.0);
        // A small window still shows one.
        assert_eq!(layout([320.0, 120.0], [116, 54], &view(9), 1).per_page, 1);
    }

    #[test]
    fn bookmarks_fill_the_gap_left_of_the_window() {
        // A 176 wide inventory centred on a 480 wide window leaves 152 px at its left: 7 columns
        // of 18 px once the 8 px margins are taken.
        let window = [152.0, 52.0, 176.0, 166.0];
        let slots = bookmark_slots([480.0, 270.0], window, 10);
        assert_eq!(slots.len(), 10);
        assert_eq!(slots[6], [8.0 + 6.0 * 18.0, 8.0, 18.0, 18.0]);
        assert_eq!(slots[7], [8.0, 26.0, 18.0, 18.0]);
        assert!(slots.iter().all(|r| r[0] + r[2] <= window[0]));
        assert_eq!(bounds(&slots), Some([8.0, 8.0, 126.0, 36.0]));
        // No room: no bookmarks drawn.
        assert!(bookmark_slots([200.0, 270.0], [12.0, 0.0, 176.0, 166.0], 3).is_empty());
    }

    #[test]
    fn clicks_find_slots_plus_and_tabs() {
        let v = view(1);
        let g = layout([480.0, 270.0], [116, 54], &v, 1);
        let d = Display::new(Some("minecraft:stick".into()))
            .input([1, 1], vec!["minecraft:oak_planks".into()])
            .output([95, 19], vec!["minecraft:stick".into()]);
        let origin = g.recipes[0].0;
        let at = |dx: f32, dy: f32| [origin[0] + dx, origin[1] + dy];
        assert_eq!(
            g.hit(at(96.0, 20.0), &[&d]),
            Hit::Slot { recipe: 0, slot: 1 }
        );
        assert_eq!(g.hit(at(0.0, 0.0), &[&d]), Hit::Slot { recipe: 0, slot: 0 });
        let plus = g.recipes[0].1;
        assert_eq!(g.hit([plus[0] + 1.0, plus[1] + 1.0], &[&d]), Hit::Plus(0));
        // A recipe without an id has no "+".
        let no_id = Display::new(None).slot(Role::Input, [1, 1], vec![]);
        assert_eq!(g.hit([plus[0] + 1.0, plus[1] + 1.0], &[&no_id]), Hit::Frame);
        let tab = g.tabs[1].1;
        assert_eq!(g.hit([tab[0] + 2.0, tab[1] + 2.0], &[&d]), Hit::Tab(1));
        assert_eq!(g.hit([1.0, 1.0], &[&d]), Hit::Outside);
    }
}
