use super::rules::*;

/// A world written by hand for one check.
struct Fake {
    kind: &'static str,
    hostile: Option<bool>,
    dimension: &'static str,
    height: f64,
    biome: &'static str,
    tags: &'static [&'static str],
    light: i32,
    time: i64,
    reason: Reason,
    by_player: Option<bool>,
    nearby: &'static [&'static str],
    draw: f64,
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            kind: "minecraft:zombie",
            hostile: Some(true),
            dimension: "minecraft:overworld",
            height: 64.0,
            biome: "minecraft:plains",
            tags: &[],
            light: 0,
            time: 18_000,
            reason: Reason::Natural,
            by_player: None,
            nearby: &[],
            draw: 0.5,
        }
    }
}

impl Facts for Fake {
    fn kind(&self) -> &str {
        self.kind
    }
    fn hostile(&self) -> Option<bool> {
        self.hostile
    }
    fn dimension(&self) -> &str {
        self.dimension
    }
    fn height(&self) -> f64 {
        self.height
    }
    fn biome(&self) -> Option<String> {
        Some(self.biome.to_owned())
    }
    fn biome_in_tag(&self, tag: &str) -> bool {
        self.tags.contains(&tag)
    }
    fn light(&self) -> Option<i32> {
        Some(self.light)
    }
    fn day_time(&self) -> Option<i64> {
        Some(self.time)
    }
    fn sees_sky(&self) -> Option<bool> {
        Some(true)
    }
    fn raining(&self) -> bool {
        false
    }
    fn thundering(&self) -> bool {
        false
    }
    fn difficulty(&self) -> Option<String> {
        Some("normal".into())
    }
    fn reason(&self) -> Reason {
        self.reason
    }
    fn by_player(&self) -> Option<bool> {
        self.by_player
    }
    fn count(&self, of: &CountOf) -> usize {
        match of {
            CountOf::ThisMob => self.nearby.iter().filter(|k| **k == self.kind).count(),
            CountOf::Mobs(m) => self
                .nearby
                .iter()
                .filter(|k| m.iter().any(|x| x == *k))
                .count(),
            _ => self.nearby.len(),
        }
    }
    fn random(&self, _: usize) -> f64 {
        self.draw
    }
}

fn spawn(json: &str) -> Vec<(usize, SpawnRule)> {
    let parsed = parse_spawn(json).unwrap();
    assert!(parsed.skipped.is_empty(), "{:?}", parsed.skipped);
    parsed.rules
}

#[test]
fn the_first_matching_rule_decides() {
    let rules = spawn(
        r#"[
          {"mob": "minecraft:zombie", "dimension": "minecraft:the_nether", "result": "allow"},
          {"hostile": true, "maxheight": 40, "result": "deny"},
          {"mob": ["minecraft:zombie", "minecraft:husk"], "result": "allow"},
          {"result": "deny"}
        ]"#,
    );
    let deep = Fake {
        height: 12.0,
        ..Fake::default()
    };
    assert_eq!(spawn_outcome(&rules, When::Position, &deep), Outcome::Deny);
    assert_eq!(
        spawn_outcome(&rules, When::Position, &Fake::default()),
        Outcome::Allow
    );
    let cow = Fake {
        kind: "minecraft:cow",
        hostile: Some(false),
        ..Fake::default()
    };
    assert_eq!(spawn_outcome(&rules, When::Position, &cow), Outcome::Deny);
}

#[test]
fn rules_apply_only_at_their_stage() {
    let rules = spawn(r#"[{"mob": "minecraft:zombie", "when": "finalize", "result": "deny"}]"#);
    assert_eq!(
        spawn_outcome(&rules, When::Position, &Fake::default()),
        Outcome::Default
    );
    assert_eq!(
        spawn_outcome(&rules, When::Finalize, &Fake::default()),
        Outcome::Deny
    );
}

#[test]
fn light_time_and_biome_bounds_are_inclusive() {
    let rules = spawn(
        r#"[{"minlight": 0, "maxlight": 7, "mintime": 13000, "maxtime": 23000,
             "biome": ["minecraft:plains", "minecraft:forest"], "result": "deny"}]"#,
    );
    let at = |light, time, biome| Fake {
        light,
        time,
        biome,
        ..Fake::default()
    };
    assert_eq!(
        spawn_outcome(&rules, When::Position, &at(7, 13_000, "minecraft:forest")),
        Outcome::Deny
    );
    assert_eq!(
        spawn_outcome(&rules, When::Position, &at(8, 18_000, "minecraft:plains")),
        Outcome::Default
    );
    // Day time wraps each day: 37000 is 13000 of the next day.
    assert_eq!(
        spawn_outcome(&rules, When::Position, &at(0, 37_000, "minecraft:plains")),
        Outcome::Deny
    );
    assert_eq!(
        spawn_outcome(&rules, When::Position, &at(0, 18_000, "minecraft:desert")),
        Outcome::Default
    );
}

#[test]
fn maxcount_denies_once_the_limit_is_reached() {
    let rules = spawn(
        r#"[{"mob": "minecraft:zombie", "maxcount": 2, "result": "default"},
            {"mob": "minecraft:zombie", "result": "deny"}]"#,
    );
    let with = |nearby| Fake {
        nearby,
        ..Fake::default()
    };
    // Below the limit the first rule matches and leaves vanilla's answer.
    assert_eq!(
        spawn_outcome(&rules, When::Position, &with(&["minecraft:zombie"])),
        Outcome::Default
    );
    assert_eq!(
        spawn_outcome(
            &rules,
            When::Position,
            &with(&["minecraft:zombie", "minecraft:zombie"])
        ),
        Outcome::Deny
    );
    let named =
        spawn(r#"[{"maxcount": {"amount": 1, "mob": "minecraft:cow"}, "result": "allow"}]"#);
    assert_eq!(
        spawn_outcome(&named, When::Position, &with(&["minecraft:cow"])),
        Outcome::Default
    );
}

#[test]
fn spawner_random_and_mod_conditions() {
    let rules = spawn(
        r#"[{"spawner": true, "result": "deny"},
            {"mod": "minecraft", "random": 0.25, "result": "deny"}]"#,
    );
    let spawner = Fake {
        reason: Reason::Spawner,
        draw: 0.9,
        ..Fake::default()
    };
    assert_eq!(
        spawn_outcome(&rules, When::Position, &spawner),
        Outcome::Deny
    );
    let lucky = Fake {
        draw: 0.2,
        ..Fake::default()
    };
    assert_eq!(spawn_outcome(&rules, When::Position, &lucky), Outcome::Deny);
    let unlucky = Fake {
        draw: 0.3,
        ..Fake::default()
    };
    assert_eq!(
        spawn_outcome(&rules, When::Position, &unlucky),
        Outcome::Default
    );
}

#[test]
fn a_rule_with_an_unknown_condition_is_left_out_and_reported() {
    let parsed = parse_spawn(
        r#"[{"mob": "minecraft:zombie", "gamestage": "two", "result": "deny"},
            {"mob": "minecraft:zombie", "when": "despawn", "result": "deny"},
            {"mob": "minecraft:zombie", "result": "deny"}]"#,
    )
    .unwrap();
    assert_eq!(parsed.rules.len(), 1);
    assert_eq!(parsed.rules[0].0, 2);
    assert_eq!(
        parsed.skipped.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(parsed.skipped[0].1.contains("gamestage"));
    assert!(parse_spawn("{}").is_err());
}

#[test]
fn experience_rules_set_multiply_add_or_deny() {
    let parsed = parse_experience(
        r#"[{"mob": "minecraft:blaze", "player": false, "result": "deny"},
            {"mob": "minecraft:blaze", "multxp": 2.5, "addxp": 1},
            {"hostile": true, "setxp": 3}]"#,
    )
    .unwrap();
    assert!(parsed.skipped.is_empty());
    let blaze = |by_player| Fake {
        kind: "minecraft:blaze",
        by_player: Some(by_player),
        ..Fake::default()
    };
    assert_eq!(experience(&parsed.rules, 10, &blaze(false)), 0);
    assert_eq!(experience(&parsed.rules, 10, &blaze(true)), 26);
    let zombie = Fake {
        by_player: Some(true),
        ..Fake::default()
    };
    assert_eq!(experience(&parsed.rules, 5, &zombie), 3);
    let cow = Fake {
        kind: "minecraft:cow",
        hostile: Some(false),
        ..Fake::default()
    };
    assert_eq!(experience(&parsed.rules, 2, &cow), 2);
}

#[test]
fn draws_are_fixed_by_their_inputs_and_spread_over_the_unit_interval() {
    assert_eq!(draw([1, 2, 3, 4]), draw([1, 2, 3, 4]));
    assert_ne!(draw([1, 2, 3, 4]), draw([1, 2, 3, 5]));
    let below_half = (0..1000u64).filter(|i| draw([*i, 7, 7, 0]) < 0.5).count();
    assert!((400..600).contains(&below_half), "{below_half}");
}
