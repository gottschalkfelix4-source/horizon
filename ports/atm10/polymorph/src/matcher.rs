//! Matching a crafting grid against every crafting recipe, so all the recipes it makes are known
//! (the game's own lookup answers only the first). Generic over the ingredient and stack types
//! and the test between them, so it is tested without a game.

/// A recipe's shape.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape<I> {
    /// A shaped recipe: `width` x `height` cells, row by row, `None` for an empty cell. It also
    /// matches mirrored left to right, as vanilla's `ShapedRecipePattern` does.
    Shaped {
        width: usize,
        height: usize,
        cells: Vec<Option<I>>,
    },
    /// A shapeless recipe: each ingredient takes one stack, in any slots.
    Shapeless(Vec<I>),
}

/// A crafting grid: `width` x `height` slots, row by row.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid<S> {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Option<S>>,
}

impl<S> Grid<S> {
    /// The smallest box holding every filled slot: `(left, top, width, height)`, or `None` for
    /// an empty grid.
    fn bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let filled = |x: usize, y: usize| self.cells[y * self.width + x].is_some();
        let cols: Vec<usize> = (0..self.width)
            .filter(|&x| (0..self.height).any(|y| filled(x, y)))
            .collect();
        let rows: Vec<usize> = (0..self.height)
            .filter(|&y| (0..self.width).any(|x| filled(x, y)))
            .collect();
        let (left, right) = (*cols.first()?, *cols.last()?);
        let (top, bottom) = (*rows.first()?, *rows.last()?);
        Some((left, top, right - left + 1, bottom - top + 1))
    }

    fn filled(&self) -> impl Iterator<Item = &S> {
        self.cells.iter().flatten()
    }
}

/// Whether `grid` makes `shape`, with `fits` the ingredient test (`Ingredient.test`).
pub fn matches<I, S>(shape: &Shape<I>, grid: &Grid<S>, fits: impl Fn(&I, &S) -> bool) -> bool {
    match shape {
        Shape::Shaped {
            width,
            height,
            cells,
        } => {
            let Some((left, top, w, h)) = grid.bounds() else {
                return false;
            };
            if (w, h) != (*width, *height) {
                return false;
            }
            let try_mirror = |mirror: bool| {
                (0..h).all(|y| {
                    (0..w).all(|x| {
                        let rx = if mirror { w - 1 - x } else { x };
                        let want = &cells[y * w + rx];
                        let have = &grid.cells[(top + y) * grid.width + left + x];
                        match (want, have) {
                            (None, None) => true,
                            (Some(i), Some(s)) => fits(i, s),
                            _ => false,
                        }
                    })
                })
            };
            try_mirror(false) || try_mirror(true)
        }
        Shape::Shapeless(ingredients) => {
            let stacks: Vec<&S> = grid.filled().collect();
            if stacks.len() != ingredients.len() {
                return false;
            }
            // Each stack must take a different ingredient: a bipartite matching, found with
            // augmenting paths (vanilla's `StackedContents` answers the same question).
            let mut owner: Vec<Option<usize>> = vec![None; ingredients.len()];
            fn assign<I, S>(
                stack: usize,
                stacks: &[&S],
                ingredients: &[I],
                fits: &dyn Fn(&I, &S) -> bool,
                owner: &mut Vec<Option<usize>>,
                seen: &mut Vec<bool>,
            ) -> bool {
                for (i, ingredient) in ingredients.iter().enumerate() {
                    if seen[i] || !fits(ingredient, stacks[stack]) {
                        continue;
                    }
                    seen[i] = true;
                    let free = match owner[i] {
                        None => true,
                        Some(other) => assign(other, stacks, ingredients, fits, owner, seen),
                    };
                    if free {
                        owner[i] = Some(stack);
                        return true;
                    }
                }
                false
            }
            (0..stacks.len()).all(|stack| {
                let mut seen = vec![false; ingredients.len()];
                assign(stack, &stacks, ingredients, &fits, &mut owner, &mut seen)
            })
        }
    }
}

/// A shaped recipe's cells from its file's `pattern` rows and its ingredients in pattern order
/// (the game lists a shaped recipe's ingredients row by row, empty cells left out). `None` when
/// the pattern and the ingredient count disagree.
pub fn shaped_from_pattern<I: Clone>(pattern: &[String], ingredients: &[I]) -> Option<Shape<I>> {
    let height = pattern.len();
    let width = pattern.iter().map(|row| row.chars().count()).max()?;
    let mut next = ingredients.iter();
    let mut cells = Vec::with_capacity(width * height);
    for row in pattern {
        let chars: Vec<char> = row.chars().collect();
        for x in 0..width {
            match chars.get(x) {
                Some(' ') | None => cells.push(None),
                Some(_) => cells.push(Some(next.next()?.clone())),
            }
        }
    }
    next.next().is_none().then_some(Shape::Shaped {
        width,
        height,
        cells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(width: usize, cells: &[&'static str]) -> Grid<&'static str> {
        Grid {
            width,
            height: cells.len() / width,
            cells: cells
                .iter()
                .map(|c| (!c.is_empty()).then_some(*c))
                .collect(),
        }
    }
    fn fits(want: &&str, have: &&str) -> bool {
        want == have
    }
    fn rows(rows: &[&str]) -> Vec<String> {
        rows.iter().map(|r| r.to_string()).collect()
    }

    #[test]
    fn a_shaped_recipe_matches_anywhere_in_the_grid_and_mirrored() {
        // An axe: planks in an L with two sticks, made in the right column of a table.
        let axe = shaped_from_pattern(
            &rows(&["XX", "X#", " #"]),
            &["plank", "plank", "plank", "stick", "stick"],
        )
        .unwrap();
        let right = grid(
            3,
            &["", "plank", "plank", "", "plank", "stick", "", "", "stick"],
        );
        assert!(matches(&axe, &right, fits));
        let mirrored = grid(
            3,
            &["plank", "plank", "", "stick", "plank", "", "stick", "", ""],
        );
        assert!(matches(&axe, &mirrored, fits));
        let wrong = grid(
            3,
            &["plank", "plank", "", "plank", "plank", "", "stick", "", ""],
        );
        assert!(!matches(&axe, &wrong, fits));
    }

    #[test]
    fn a_shapeless_recipe_needs_each_ingredient_once_in_any_slot() {
        let dye = Shape::Shapeless(vec!["blue", "yellow"]);
        assert!(matches(&dye, &grid(2, &["", "yellow", "blue", ""]), fits));
        assert!(!matches(&dye, &grid(2, &["blue", "blue", "", ""]), fits));
        assert!(!matches(
            &dye,
            &grid(2, &["blue", "yellow", "blue", ""]),
            fits
        ));
    }

    #[test]
    fn a_shapeless_match_reassigns_ingredients_that_accept_several_items() {
        // "any wood" accepts oak and birch, "oak" only oak: oak must go to "oak".
        let any = |want: &&str, have: &&str| *want == "any wood" || want == have;
        let shape = Shape::Shapeless(vec!["any wood", "oak"]);
        assert!(matches(&shape, &grid(2, &["oak", "birch", "", ""]), any));
        assert!(!matches(&shape, &grid(2, &["birch", "birch", "", ""]), any));
    }

    #[test]
    fn a_pattern_that_disagrees_with_its_ingredients_is_refused() {
        assert!(shaped_from_pattern(&rows(&["XX"]), &["plank"]).is_none());
        assert!(shaped_from_pattern(&rows(&["X"]), &["plank", "plank"]).is_none());
    }
}
