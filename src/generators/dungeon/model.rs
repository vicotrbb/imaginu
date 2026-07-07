//! The deterministic dungeon model. Rooms, corridors, doors and spawns are a
//! pure function of (`DungeonParams`, seed) — the dungeon analog of the world
//! "seam law": every room/corridor coordinate snaps to an integer meter so the
//! layout is bit-exact across rebuilds and independent of iteration order.

use glam::Vec3;

use crate::palette::Palette;
use crate::recipe::{DungeonParams, DungeonTheme};

/// A room's role in the dungeon (drives spawns + dressing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomKind {
    Entrance,
    Normal,
    Boss,
    Treasure,
}

impl RoomKind {
    pub fn name(self) -> &'static str {
        match self {
            RoomKind::Entrance => "entrance",
            RoomKind::Normal => "normal",
            RoomKind::Boss => "boss",
            RoomKind::Treasure => "treasure",
        }
    }
}

/// An axis-aligned room box. `min`/`max` are in world meters, all integer.
/// The floor sits at `min.y` (== 0) and the ceiling at `max.y`.
#[derive(Clone, Debug)]
pub struct Room {
    pub id: usize,
    pub kind: RoomKind,
    pub min: Vec3,
    pub max: Vec3,
}

impl Room {
    /// Footprint center (may sit on a half-meter for odd spans).
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
    /// Integer-snapped footprint center used for corridor routing/doors.
    pub fn center_i(&self) -> Vec3 {
        let c = self.center();
        Vec3::new(c.x.round(), 0.0, c.z.round())
    }
}

/// A corridor connecting two rooms. `path` is an integer-aligned polyline that
/// runs door-to-door (L-shaped for orthogonal themes).
#[derive(Clone, Debug)]
pub struct Corridor {
    pub a: usize,
    pub b: usize,
    pub path: Vec<Vec3>,
}

/// A doorway where a corridor pierces a room wall.
#[derive(Clone, Debug)]
pub struct Door {
    pub room: usize,
    pub corridor: usize,
    pub pos: Vec3,
}

/// What a spawn point spawns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnKind {
    Player,
    Enemy,
    Loot,
    Boss,
}

impl SpawnKind {
    pub fn name(self) -> &'static str {
        match self {
            SpawnKind::Player => "player",
            SpawnKind::Enemy => "enemy",
            SpawnKind::Loot => "loot",
            SpawnKind::Boss => "boss",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpawnPoint {
    pub kind: SpawnKind,
    pub pos: Vec3,
}

/// The pure-of-seed dungeon layout. Geometry (D3) and the manifest (D4) are
/// built from this — never the other way around.
pub struct DungeonModel {
    pub p: DungeonParams,
    pub pal: Palette,
    pub rooms: Vec<Room>,
    pub corridors: Vec<Corridor>,
    pub doors: Vec<Door>,
    pub spawns: Vec<SpawnPoint>,
    /// Overall AABB (min, max) covering rooms + corridors.
    pub bounds: (Vec3, Vec3),
}

impl DungeonModel {
    pub fn new(p: &DungeonParams, pal: &Palette) -> Result<Self, String> {
        // Clamp hostile input so absurd recipes can never hang or OOM.
        let mut p = p.clone();
        p.rooms = p.rooms.map(|r| r.clamp(1, 60));
        p.loops = p.loops.clamp(0.0, 1.0);
        p.density = p.density.clamp(0.0, 1.0);
        p.detail = p.detail.clamp(0.5, 2.0);

        let pal = crate::palette::by_name(pal.name);
        super::layout::build(p, pal)
    }

    /// Ceiling height in meters for a theme (integer).
    pub fn ceiling(theme: DungeonTheme) -> f32 {
        match theme {
            DungeonTheme::Crypt => 4.0,
            DungeonTheme::Cavern => 6.0,
            DungeonTheme::Sewer => 4.0,
            DungeonTheme::Mine => 4.0,
            DungeonTheme::Temple => 6.0,
            DungeonTheme::Fortress => 5.0,
        }
    }
}
