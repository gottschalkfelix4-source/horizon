use super::*;

impl Game<'_> {
    /// The container slots (of `size`) that the mod's screen for the block at
    /// `pos` does not show: the server's menu does not have them either, so a
    /// shift-click never moves an item where the player cannot see it.
    pub(in crate::game) fn native_hidden_slots(&self, pos: Pos, size: usize) -> Vec<usize> {
        let Some(menu) = self.mod_menu_at(pos) else {
            return Vec::new();
        };
        let block = self
            .res
            .facts()
            .get(self.world.block(pos.0, pos.1, pos.2))
            .name
            .to_string();
        let mut screen = mod_api::RecipeBookLayout::registered_screen(menu);
        if screen.is_none() {
            self.native_screen(&block, &mut screen);
        }
        if screen.is_none() {
            screen = default_grid_screen(menu);
        }
        let Some(screen) = screen else {
            return Vec::new();
        };
        (0..size)
            .filter(|&i| screen.slots.get(i).is_none_or(Option::is_none))
            .collect()
    }
    /// A menu opened on this client with its slots under `key`, on block
    /// `target`: a book-enabled menu uses its startup-validated registered screen;
    /// other mod menus obtain their screen through `native_screen`.
    /// `extra` is what the server sent for this window first.
    pub(in crate::game) fn open_native_screen(
        &mut self,
        key: Pos,
        target: Option<Pos>,
        window: i32,
        menu: &str,
        extra: Option<horizon_protocol::menu_extra::Message>,
    ) {
        self.clear_mod_book_placement_screen();
        self.mod_screen = None;
        let Some(t) = target else {
            return;
        };
        let block = self
            .res
            .facts()
            .get(self.world.block(t.0, t.1, t.2))
            .name
            .to_string();
        if block.starts_with("minecraft:") {
            return;
        }
        // Only when this is the menu the clicked block opens: `target` outlives the
        // use that set it, so another menu opened later (a command's, another
        // mod's) would otherwise get the last clicked machine's screen. Asked
        // here, as the window's world has no mod containers.
        let mut container = None;
        self.native_block_entity_container(&block, &mut container);
        let opens = container.and_then(|c| mod_menu(c.menu.as_deref()?).map(|(m, _)| m));
        if opens != Some(menu) {
            return;
        }
        let mut screen = mod_api::RecipeBookLayout::registered_screen(menu);
        if screen.is_none() {
            self.native_screen(&block, &mut screen);
        }
        if screen.is_none() {
            screen = default_grid_screen(menu);
        }
        if let Some(screen) = screen {
            self.mod_screen = Some(OpenNativeScreen {
                layout_revision: {
                    static NEXT: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(1);
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                },
                key,
                window,
                block,
                screen,
                data: Vec::new(),
                extra,
            });
            self.begin_mod_book_placement();
            self.ensure_mod_book_catalog();
        }
    }
}

/// The screen a content mod's menu type gets when its mod gives none: the container's slots
/// in a grid under the title (nine columns up to 54 slots, then wider, never past nine rows),
/// the player's inventory below, in vanilla's chest layout. None for a menu that is not a
/// content mod's menu type.
fn default_grid_screen(menu: &str) -> Option<mod_api::Screen> {
    let slots = horizon_client::containers::mod_menu_type(menu)?.slots;
    Some(grid_screen(slots))
}

fn grid_screen(slots: usize) -> mod_api::Screen {
    let columns = if slots <= 54 { 9 } else { slots.div_ceil(9) };
    let rows = slots.div_ceil(columns).max(1);
    let width = (14 + 18 * columns).max(176) as f32;
    let inventory_y = (rows * 18 + 32) as f32;
    mod_api::Screen {
        width,
        height: inventory_y + 82.0,
        slots: (0..slots)
            .map(|i| Some([(8 + 18 * (i % columns)) as f32, (18 + 18 * (i / columns)) as f32]))
            .collect(),
        inventory_y,
        inventory_x: 8.0 + (width - 176.0) / 2.0,
        ..Default::default()
    }
}

#[cfg(test)]
mod grid_tests {
    use super::grid_screen;

    #[test]
    fn a_default_grid_matches_vanillas_chest_layout() {
        // Vanilla's 9x6 chest: slots from (8, 18), the inventory at y 140, 222 high, 176 wide.
        let chest = grid_screen(54);
        assert_eq!(chest.slots[0], Some([8.0, 18.0]));
        assert_eq!(chest.slots[53], Some([152.0, 108.0]));
        assert_eq!(
            (chest.width, chest.height, chest.inventory_y, chest.inventory_x),
            (176.0, 222.0, 140.0, 8.0)
        );
        // 256 slots stay within nine rows, wider than a chest.
        let big = grid_screen(256);
        assert_eq!(big.slots.len(), 256);
        assert_eq!(big.slots[255], Some([8.0 + 18.0 * 23.0, 18.0 + 18.0 * 8.0]));
        assert_eq!(big.width, 14.0 + 18.0 * 29.0);
    }
}
