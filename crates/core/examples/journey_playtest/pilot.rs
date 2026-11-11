use blah_core::{Dir, Game, Herb, LandmarkKind, LogKind, Maneuver, Relic, Step, VIEW_RADIUS, WeaponItem};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

type Point = (i64, i64);

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub outcome: &'static str,
    pub actions: usize,
    pub moves: usize,
    pub camps: usize,
    pub rests: usize,
    pub picks: usize,
    pub brews: usize,
    pub claims: usize,
    pub relics: usize,
    pub trades: usize,
    pub drinks: usize,
    pub deaths: u32,
    pub relic_uses: usize,
    pub milestones: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Outbound,
    Home,
    Again,
}

#[derive(Clone, Copy)]
enum Site {
    Herb(Herb),
    Cache,
    Landmark(LandmarkKind),
}

fn distance(a: Point, b: Point) -> i64 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

fn observe(game: &Game, terrain: &mut BTreeMap<Point, bool>, sites: &mut BTreeMap<Point, Site>, trails: bool) {
    let (x, y) = game.position();
    for dy in -VIEW_RADIUS..=VIEW_RADIUS {
        for dx in -VIEW_RADIUS..=VIEW_RADIUS {
            let at = (x + dx, y + dy);
            terrain.insert(at, game.world().tile(at.0, at.1).biome.biome().passable);
            if let Some(herb) = game.herb_at(at.0, at.1) {
                sites.insert(at, Site::Herb(herb.herb));
            }
            if game.cache_at(at.0, at.1).is_some() {
                sites.insert(at, Site::Cache);
            }
            if let Some(landmark) = game.landmark_at(at.0, at.1) {
                sites.insert(at, Site::Landmark(landmark.kind));
            }
        }
    }
    for site in game.known_landmarks() {
        sites.insert(site.position, Site::Landmark(site.kind));
    }
    if trails {
        if let Some(at) = game.cache_trail() {
            sites.entry(at).or_insert(Site::Cache);
        }
        for herb in Herb::ALL {
            if let Some(at) = game.herb_trail(herb) {
                sites.entry(at).or_insert(Site::Herb(herb));
            }
        }
    }
}

/// Known ground only; no attacks or knowledge of unseen monsters in path planning.
fn paths(game: &Game, terrain: &BTreeMap<Point, bool>) -> BTreeMap<Point, (usize, Option<Dir>)> {
    let start = game.position();
    let occupied: BTreeSet<_> = game.monsters_in_view().map(|m| m.position).collect();
    let mut paths = BTreeMap::from([(start, (0, None))]);
    let mut queue = VecDeque::from([start]);
    while let Some(at) = queue.pop_front() {
        let (length, first) = paths[&at];
        for dir in Dir::ALL {
            let (dx, dy) = dir.delta();
            let next = (at.0 + dx, at.1 + dy);
            if terrain.get(&next) != Some(&true) || occupied.contains(&next) || paths.contains_key(&next) {
                continue;
            }
            paths.insert(next, (length + 1, first.or(Some(dir))));
            queue.push_back(next);
        }
    }
    paths
}

fn frontier(at: Point, terrain: &BTreeMap<Point, bool>) -> bool {
    [-VIEW_RADIUS, VIEW_RADIUS].into_iter().any(|edge| {
        (-VIEW_RADIUS..=VIEW_RADIUS).any(|offset| {
            !terrain.contains_key(&(at.0 + edge, at.1 + offset)) || !terrain.contains_key(&(at.0 + offset, at.1 + edge))
        })
    })
}

fn record(report: &mut Report, game: &Game, text: &str) {
    report.milestones.push(format!("action {} turn {}: {text}", report.actions, game.turn()));
}

fn reload(game: &mut Game) {
    let encoded = serde_json::to_vec(&game.snapshot()).expect("encode actual journey");
    let restored = Game::from_snapshot(serde_json::from_slice(&encoded).unwrap()).expect("restore actual journey");
    assert_eq!(*game, restored, "a journey milestone must survive serialization");
    *game = restored;
}

pub fn run(seed: u64, budget: usize) -> Report {
    run_with_trails(seed, budget, true)
}

pub fn run_with_trails(seed: u64, budget: usize, trails: bool) -> Report {
    journey(seed, budget, trails, false)
}

#[cfg(test)]
pub fn run_reloading(seed: u64, budget: usize) -> Report {
    journey(seed, budget, true, true)
}

fn journey(seed: u64, budget: usize, trails: bool, reload_each: bool) -> Report {
    let mut game = Game::new(seed);
    game.prepare(Maneuver::default()).expect("fresh home is safe");
    let mut report = Report { outcome: "budget", ..Report::default() };
    let mut terrain = BTreeMap::new();
    let mut sites = BTreeMap::new();
    let mut spent = BTreeSet::new();
    let mut phase = Phase::Outbound;
    let mut returning_with_reward = false;
    let mut used_reward = false;
    let mut claimed_on_return = 0;
    let mut stalls = 0;
    let mut waypoint = None;
    while report.actions < budget {
        if reload_each {
            reload(&mut game);
        }
        observe(&game, &mut terrain, &mut sites, trails);
        if game.deaths() > 0 {
            report.outcome = "died";
            break;
        }
        if phase == Phase::Outbound && (report.claims > 0 || !game.relics().is_empty()) {
            phase = Phase::Home;
            waypoint = None;
            returning_with_reward = true;
            record(&mut report, &game, "reward acquired; return to prepare the next outing");
            reload(&mut game);
        }
        if game.wanderer().hp <= 10 && phase != Phase::Home {
            phase = Phase::Home;
            waypoint = None;
            record(&mut report, &game, "low Health; retreat home");
        }
        if phase == Phase::Home && game.position() == game.home() {
            if game.can_rest() {
                game.recover_focus();
                report.rests += 1;
                report.actions += 1;
                continue;
            }
            if !game.can_prepare() {
                report.outcome = "home-blocked";
                break;
            }
            if returning_with_reward {
                if let Some(item) = game.inventory().iter().copied().find(|i| WeaponItem::CACHE_REWARDS.contains(i))
                    && game.draughts() < 2
                {
                    assert_eq!(game.trade_weapon(item), Step::Waited);
                    report.trades += 1;
                    report.actions += 1;
                    record(&mut report, &game, "trade unsuitable reward for supplies");
                }
                phase = Phase::Again;
                claimed_on_return = report.claims;
                record(&mut report, &game, "rested at home; begin second outing with earned supplies/relics");
                reload(&mut game);
                if report.actions >= budget {
                    break;
                }
            } else {
                report.outcome = "retreated";
                break;
            }
        }
        if phase == Phase::Again && used_reward && report.claims > claimed_on_return {
            report.outcome = "complete";
            record(&mut report, &game, "earned reward used on second outing; another cache claimed");
            break;
        }
        let watched = game.monsters_in_view().next().is_some();
        let before = game.turn();
        let old_log = game.log().total();
        let mut acted = false;
        if game.wanderer().hp <= 18 && game.salves() > 0 {
            acted = game.use_salve() == Step::Waited;
        } else if game.wanderer().focus <= 3 && game.draughts() > 0 {
            acted = game.drink_draught() == Step::Waited;
            if acted {
                report.drinks += 1;
                used_reward |= phase == Phase::Again && report.trades > 0;
                record(&mut report, &game, "drink carried draught rather than return home");
            }
        } else if !watched
            && (game.wanderer().focus < 12
                || (Herb::ALL.into_iter().filter(|&h| game.herbs(h) > 0).count() >= 2 && game.draughts() < 2))
        {
            let brews = game.draughts();
            let ember = phase == Phase::Again
                && game.has_relic(Relic::HearthEmber)
                && game.wanderer().hp < game.wanderer().max_hp() - 1;
            acted = game.camp() == Step::Waited;
            if acted {
                report.camps += 1;
                if ember {
                    used_reward = true;
                    report.relic_uses += 1;
                    record(&mut report, &game, "Hearth ember improves field recovery");
                }
                if game.draughts() > brews {
                    report.brews += 1;
                    record(&mut report, &game, "brew in the field without returning home");
                }
            }
        } else if watched && !game.sneaking() && (game.wanderer().focus > 0 || game.has_relic(Relic::Hushstone)) {
            acted = game.toggle_sneak() == Step::Waited;
            if acted && phase == Phase::Again && game.has_relic(Relic::Hushstone) {
                used_reward = true;
                report.relic_uses += 1;
                record(&mut report, &game, "Hushstone saves the Focus for entering sneak");
            }
        } else if game.sneaking() && (!watched || game.wanderer().focus == 0) {
            acted = game.toggle_sneak() == Step::Waited;
        }
        if !acted && phase != Phase::Home && !spent.contains(&game.position()) {
            let at = game.position();
            match sites.get(&at) {
                Some(Site::Cache) if game.cache_at(at.0, at.1).is_some_and(|c| !c.claimed) => {
                    acted = game.claim_cache() == Step::Waited;
                    if acted {
                        report.claims += 1;
                        record(&mut report, &game, "claim cache without killing guards");
                    }
                }
                Some(Site::Herb(herb)) if game.herbs(*herb) < 2 => {
                    acted = game.gather() == Step::Waited;
                    report.picks += usize::from(acted);
                }
                Some(Site::Landmark(kind)) => {
                    acted = game.use_landmark() == Step::Waited;
                    if acted {
                        record(&mut report, &game, &format!("use {} (relics: {:?})", kind.name(), game.relics()));
                    }
                }
                _ => {}
            }
            if acted {
                spent.insert(at);
                waypoint = None;
            }
        }
        if !acted {
            let routes = paths(&game, &terrain);
            let direction = if phase == Phase::Home {
                routes.get(&game.home()).and_then(|p| p.1)
            } else {
                if waypoint.is_some_and(|at| at == game.position() || !routes.contains_key(&at)) {
                    waypoint = None;
                }
                let target = sites
                    .iter()
                    .filter(|(at, site)| {
                        !spent.contains(at)
                            && **at != game.position()
                            && match site {
                                Site::Herb(herb) => game.herbs(*herb) < 1,
                                Site::Cache => game.cache_at(at.0, at.1).is_none_or(|c| !c.claimed),
                                Site::Landmark(LandmarkKind::Spring) => game.draughts() < 2 || game.salves() < 2,
                                Site::Landmark(_) => true,
                            }
                    })
                    .filter_map(|(at, site)| {
                        let route = routes.get_key_value(at).or_else(|| {
                            routes
                                .iter()
                                .filter(|(point, route)| route.1.is_some() && frontier(**point, &terrain))
                                .min_by_key(|(point, route)| (distance(**point, *at), route.0, **point))
                        });
                        route.map(|(point, route)| (at, site, point, route))
                    })
                    .min_by_key(|(at, site, point, route)| {
                        let priority = match site {
                            Site::Herb(_) => 0,
                            Site::Cache => 4,
                            Site::Landmark(_) => 0,
                        };
                        (route.0 + distance(**point, **at) as usize + priority, **at)
                    });
                if waypoint.is_none() {
                    waypoint = target.map(|(_, _, point, _)| *point).or_else(|| {
                        routes
                            .iter()
                            .filter(|(at, route)| route.1.is_some() && frontier(**at, &terrain))
                            .min_by_key(|(at, route)| (route.0, distance(**at, game.home()), **at))
                            .map(|(at, _)| *at)
                    });
                }
                waypoint.and_then(|at| routes.get(&at).and_then(|p| p.1))
            };
            if let Some(dir) = direction {
                let result = game.step(dir);
                assert_ne!(result, Step::Attacked, "the non-combat pilot must not bump-attack");
                report.moves += usize::from(result == Step::Moved);
            } else {
                report.outcome = "no-route";
                break;
            }
        }
        report.actions += 1;
        let added = (game.log().total() - old_log) as usize;
        if game.log().entries().rev().take(added).any(|e| e.kind == LogKind::Kill) {
            panic!("the non-combat pilot must not attack");
        }
        stalls = if game.turn() == before { stalls + 1 } else { 0 };
        if stalls >= 3 {
            report.outcome = "stalled";
            record(
                &mut report,
                &game,
                &format!(
                    "blocked at {:?}, Health {}, Focus {}: {:?}",
                    game.position(),
                    game.wanderer().hp,
                    game.wanderer().focus,
                    game.log().entries().last().map(|entry| &entry.text)
                ),
            );
            break;
        }
    }
    report.relics = game.relics().len();
    report.deaths = game.deaths();
    reload(&mut game);
    report
}
