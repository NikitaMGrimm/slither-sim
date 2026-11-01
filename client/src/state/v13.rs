//! Protocol-13 views mapped into the same received/presentation inputs as 19.
use super::*;
use slither_protocol::v13::{self, Message as Legacy};

/// The shared appearance header, independent of the two body grammars.
pub(super) struct AppearanceHeader<'a> {
    pub id: u16,
    pub heading: u32,
    pub raw_direction: u8,
    pub wanted_heading: u32,
    pub speed: u16,
    pub fraction: u32,
    pub skin: u8,
    pub head: [u32; 2],
    pub name: &'a [u8],
    pub custom_skin: &'a [u8],
    pub cosmetic: u8,
}

impl<'a> From<v19::SnakeAppearanceRef<'a>> for AppearanceHeader<'a> {
    fn from(raw: v19::SnakeAppearanceRef<'a>) -> Self {
        Self {
            id: raw.id,
            heading: raw.heading,
            raw_direction: raw.raw_direction,
            wanted_heading: raw.wanted_heading,
            speed: raw.speed,
            fraction: raw.fraction,
            skin: raw.skin,
            head: raw.head,
            name: raw.name,
            custom_skin: raw.custom_skin,
            cosmetic: raw.cosmetic,
        }
    }
}
impl<'a> From<v13::SnakeAppearanceRef<'a>> for AppearanceHeader<'a> {
    fn from(raw: v13::SnakeAppearanceRef<'a>) -> Self {
        Self {
            id: raw.id,
            heading: raw.heading,
            raw_direction: raw.raw_direction,
            wanted_heading: raw.wanted_heading,
            speed: raw.speed,
            fraction: raw.fraction,
            skin: raw.skin,
            head: raw.head,
            name: raw.name,
            custom_skin: raw.custom_skin,
            cosmetic: raw.cosmetic,
        }
    }
}

impl ClientState {
    pub(crate) fn prepare_single_v13_food(
        &self,
        command: &[u8],
    ) -> Result<Option<PreparedFood>, StateError> {
        if self.setup.as_ref().is_none_or(|s| s.protocol != 13)
            || !matches!(command.first(), Some(b'F' | b'f' | b'b'))
        {
            return Ok(None);
        }
        let raw = v13::decode(command, DecodeAdmission::ClientPresentation)
            .map_err(|_| StateError::InvalidPacket(command[0]))?;
        let setup = self.setup.as_ref().expect("selected protocol");
        match raw {
            Legacy::FoodSpawn(raw) => {
                let mut prepared =
                    self.prepare_legacy_food(setup, raw.position, raw.size, raw.color, raw.rapid)?;
                prepared.update_added_cache = true;
                Ok(Some(prepared))
            }
            Legacy::FoodBatch(raw) if raw.foods().len() == 1 => {
                let food = raw.foods().next().expect("one food");
                self.prepare_legacy_food(setup, food.position, food.size, food.color, true)
                    .map(Some)
            }
            _ => Ok(None),
        }
    }

    pub(super) fn apply_v13(
        &mut self,
        command: CommandRef<'_>,
        admission: DecodeAdmission,
        mut events: Option<&mut Vec<PresentationEvent>>,
    ) -> Result<bool, StateError> {
        let code = command.bytes[0];
        let raw =
            v13::decode_command(command, admission).map_err(|_| StateError::InvalidPacket(code))?;
        let setup = self.setup.clone();
        let setup = || setup.as_ref().ok_or(StateError::MissingSetup);
        // Scales, relative-byte origins, body deltas and turn directions below
        // are protocol interpretations, not measured physics. See the catalog:
        // https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-protocol13-catalog/REPORT.md
        let common = match raw {
            Legacy::SnakeAppearance(raw) => {
                self.appearance(
                    raw.into(),
                    raw.body.first,
                    raw.body.deltas().map(v19::BodyDelta::HalfUnits),
                    setup()?,
                    events,
                )?;
                // Every pair, including the final one, is a half-unit delta in 13.
                // Retain the absolute front without a centering round trip.
                if let Some([x, y]) = raw.body.first {
                    let mut front = Point {
                        x: scalar::position5(x),
                        y: scalar::position5(y),
                    };
                    for [x, y] in raw.body.deltas() {
                        front.x += scalar::delta_half(x);
                        front.y += scalar::delta_half(y);
                    }
                    self.legacy_fronts.insert(raw.id, front);
                } else {
                    self.legacy_fronts.remove(&raw.id);
                }
                return Ok(true);
            }
            Legacy::Anchor(raw) => {
                let target = self.legacy_target(raw.id);
                if target != v19::Target::Own && !self.snakes.contains_key(&raw.id) {
                    return Ok(true);
                }
                let point = match raw.coordinates {
                    v13::Coordinates::Absolute([x, y]) => Point {
                        x: f64::from(x),
                        y: f64::from(y),
                    },
                    v13::Coordinates::Relative([x, y]) => {
                        let front = self
                            .legacy_fronts
                            .get(&raw.id)
                            .ok_or(StateError::MissingAnchor(raw.id))?;
                        checked_point(
                            front.x + f64::from(i16::from(x) - 128),
                            front.y + f64::from(i16::from(y) - 128),
                        )?
                    }
                    _ => return Err(StateError::InvalidPacket(code)),
                };
                self.anchor_at(
                    v19::AnchorRef {
                        target,
                        direction: None,
                        absolute: None,
                        grew: raw.grew,
                        fraction: raw.fraction,
                    },
                    setup()?,
                    events,
                    Some(point),
                )?;
                self.legacy_fronts.insert(raw.id, point);
                return Ok(true);
            }
            Legacy::Motion(raw) => Message::Motion(v19::MotionRef {
                command: raw.command,
                target: self.legacy_target(raw.id),
                heading: raw.heading,
                wanted_heading: raw.wanted_heading,
                speed: raw.speed,
                direction: raw.direction,
            }),
            Legacy::FoodBatch(raw) => {
                for food in raw.foods() {
                    let input = self.insert_legacy_food(
                        setup()?,
                        food.position,
                        food.size,
                        food.color,
                        true,
                    )?;
                    if let Some(events) = &mut events {
                        events.push(PresentationEvent::FoodSpawn(input));
                    }
                }
                return Ok(true);
            }
            Legacy::FoodSpawn(raw) => {
                let input = self.insert_legacy_food(
                    setup()?,
                    raw.position,
                    raw.size,
                    raw.color,
                    raw.rapid,
                )?;
                self.added_sector = Some(input.sector);
                self.added_color = raw.color;
                if let Some(events) = events {
                    events.push(PresentationEvent::FoodSpawn(input));
                }
                return Ok(true);
            }
            Legacy::FoodEaten(raw) => {
                let key = legacy_food_key(raw.position);
                self.foods.remove(&key);
                self.eaten_sector = Some(legacy_sector(setup()?, raw.position)?);
                if let Some(id) = raw.eater {
                    self.eater_id = Some(id);
                }
                if let Some(events) = events {
                    events.push(PresentationEvent::FoodConsume {
                        key,
                        eater_id: raw.eater,
                    });
                }
                return Ok(true);
            }
            Legacy::Setup(raw) => Message::Setup(raw),
            Legacy::SnakeRemoval(raw) => {
                if self.own_snake_id != Some(raw.id) {
                    self.legacy_fronts.remove(&raw.id);
                }
                Message::SnakeRemoval(raw)
            }
            Legacy::Mass { id, fraction } => Message::Mass { id, fraction },
            Legacy::Tail { id, fraction } => Message::Tail { id, fraction },
            Legacy::SectorClear(raw) => Message::SectorClear(raw),
            Legacy::PreyAppearance(raw) => Message::PreyAppearance(raw),
            Legacy::PreyRemoval { id, eater } => Message::PreyRemoval { id, eater },
            Legacy::PreyMotion(raw) => Message::PreyMotion(raw),
            Legacy::Radius(raw) => Message::Radius(raw),
            Legacy::ScoreAdjustment(raw) => Message::ScoreAdjustment(raw),
            // Disabled lanes and unsupported v19-only opcodes stay opaque.
            _ => return Ok(false),
        };
        self.apply_message(common, code, events)
    }

    fn legacy_target(&self, id: u16) -> v19::Target {
        if self.own_snake_id == Some(id) {
            v19::Target::Own
        } else {
            v19::Target::Id(id)
        }
    }

    fn insert_legacy_food(
        &mut self,
        setup: &Setup,
        xy: [u16; 2],
        size: u8,
        color: u8,
        rapid: bool,
    ) -> Result<FoodSpawnInput, StateError> {
        let prepared = self.prepare_legacy_food(setup, xy, size, color, rapid)?;
        self.foods.insert(prepared.spawn.key, prepared.food);
        Ok(prepared.spawn)
    }

    fn prepare_legacy_food(
        &self,
        setup: &Setup,
        xy: [u16; 2],
        size: u8,
        color: u8,
        rapid: bool,
    ) -> Result<PreparedFood, StateError> {
        // Keep exact integer coordinates as keys: quantizing onto 19's 256-cell
        // sector grid can merge distinct 13 pellets in a 300-unit sector.
        let key = legacy_food_key(xy);
        let sector = legacy_sector(setup, xy)?;
        if !self.foods.contains_key(&key) && self.foods.len() >= MAX_FOODS {
            return Err(StateError::Limit);
        }
        let absolute_position = Point {
            x: f64::from(xy[0]),
            y: f64::from(xy[1]),
        };
        Ok(PreparedFood {
            update_added_cache: false,
            food: Food {
                position: centered(setup, absolute_position.x, absolute_position.y)?,
                size: f64::from(size) / 5.,
                color,
                sector,
            },
            spawn: FoodSpawnInput {
                key,
                sector,
                absolute_position,
                size_byte: size,
                base_size: f64::from(size) / 5.,
                raw_color: color,
                rapid,
            },
        })
    }
}

fn legacy_food_key([x, y]: [u16; 2]) -> u32 {
    (u32::from(x) << 16) | u32::from(y)
}
fn legacy_sector(setup: &Setup, [x, y]: [u16; 2]) -> Result<(u8, u8), StateError> {
    Ok((
        u8::try_from(x / setup.sector_size).map_err(|_| StateError::Coordinate)?,
        u8::try_from(y / setup.sector_size).map_err(|_| StateError::Coordinate)?,
    ))
}
