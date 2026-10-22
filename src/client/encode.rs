//! Protocol-19 packet encoders for the simulator's server side.
//!
//! Layouts follow `tools/fidelity/slither_protocol.py` and `slither-headless.mjs`.
//! See `docs/provenance.md` for reference sources and coverage.
use crate::{
    client::{Minimap, simulation::PlayerAppearance},
    model::{Point, Snake},
    physics,
    prey::Prey,
};
use std::f64::consts::{PI, TAU};

/// Wire offset of the arena center. 32550 * 256 / 300 is an integer, so the
/// simulator's 300/256 food grid maps exactly onto wire sector coordinates.
pub(crate) const GRID_CENTER: u32 = 32550;
pub(crate) const SECTOR_SIZE: u16 = 300;
pub(crate) const PROTOCOL: u8 = 19;
/// Largest radius whose rim stays inside absolute u16 anchor coordinates.
pub(crate) const MAX_RADIUS: f64 = 32000.;
/// A packet longer than this cannot carry a two-byte frame length prefix.
const MAX_PACKET: usize = (32 << 8) - 1;
const U24: f64 = 16777215.;

fn u16be(value: u16) -> [u8; 2] {
    value.to_be_bytes()
}
fn u24be(value: u32) -> [u8; 3] {
    let [_, a, b, c] = value.to_be_bytes();
    [a, b, c]
}
fn wire(value: f64) -> f64 {
    value + f64::from(GRID_CENTER)
}
fn checked(value: f64, max: f64, what: &str) -> Result<f64, String> {
    if value.is_finite() && (0. ..=max).contains(&value) {
        Ok(value)
    } else {
        Err(format!("{what} is outside the protocol-19 wire range"))
    }
}

/// 24-bit angle used by appearances and prey.
pub(crate) fn angle_u24(angle: f64) -> u32 {
    (angle.rem_euclid(TAU) / TAU * U24).round() as u32 % 16777216
}
/// Stock turn direction from `heading` toward `target` along the shorter way:
/// 1 decreases the angle, 2 increases it and 0 means no turn.
pub(crate) fn direction(heading: f64, target: f64) -> u8 {
    let turn = (target - heading + PI).rem_euclid(TAU) - PI;
    if turn < 0. {
        1
    } else if turn > 0. {
        2
    } else {
        0
    }
}
/// One-byte angle used by motion packets.
pub(crate) fn angle_byte(angle: f64) -> u8 {
    ((angle.rem_euclid(TAU) / TAU * 256.).round() as u32 % 256) as u8
}
/// 16-bit anchor angle.
pub(crate) fn angle_u16(angle: f64) -> u16 {
    ((angle.rem_euclid(TAU) / TAU * 65536.).round() as u32 % 65536) as u16
}
/// Motion speed byte: world units/s converted to protocol `sp`, times 18.
pub(crate) fn speed_byte(speed: f64) -> Result<u8, String> {
    Ok(checked(
        (speed * physics::SPEED_UNIT_SECONDS * 18.).round(),
        255.,
        "Speed",
    )? as u8)
}
pub(crate) fn speed_u16(speed: f64) -> Result<u16, String> {
    Ok(checked((speed * 32.).round(), 65535., "Speed")? as u16)
}
/// Fractional mass toward the next segment; the wire saturates at one.
pub(crate) fn fam_u24(fam: f64) -> u32 {
    (fam.clamp(0., 1.) * U24).round() as u32
}
fn coord_u24(value: f64) -> Result<[u8; 3], String> {
    Ok(u24be(
        checked((wire(value) * 5.).round(), U24, "Coordinate")? as u32,
    ))
}
fn coord_u16(value: f64) -> Result<[u8; 2], String> {
    Ok(u16be(
        checked(wire(value).round(), 65535., "Coordinate")? as u16
    ))
}
fn with_id(code: u8, own: bool, id: u16) -> Vec<u8> {
    let mut packet = vec![code];
    if !own {
        packet.extend(u16be(id));
    }
    packet
}

/// Whether a packet can share a frame behind a two-byte length prefix.
pub(crate) fn frameable(packet: &[u8]) -> bool {
    !packet.is_empty() && packet.len() <= MAX_PACKET
}

/// Two-byte elapsed-ms header, then one raw packet or length-prefixed packets.
pub(crate) fn frame(elapsed_ms: u16, packets: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let mut out = u16be(elapsed_ms).to_vec();
    match packets {
        [] => return Err("Empty frame".into()),
        [packet] if packet.first().is_some_and(|&code| code >= 32) => out.extend(packet),
        _ => {
            for packet in packets {
                if !frameable(packet) {
                    return Err("Packet length cannot be framed".into());
                }
                out.extend(u16be(packet.len() as u16));
                out.extend(packet);
            }
        }
    }
    Ok(out)
}

/// Server number in setup bytes 25..27. Stock clients display it as the server;
/// it never identifies the player's snake (the first full appearance does).
pub(crate) const SERVER_NUMBER: u16 = 1;

/// Setup packet with the constants the simulator's physics uses.
pub(crate) fn setup(radius: f64) -> Result<Vec<u8>, String> {
    let mut p = vec![b'a'];
    p.extend(u24be(GRID_CENTER));
    p.extend(u16be(physics::MAX_SEGMENTS as u16));
    p.extend(u16be(SECTOR_SIZE));
    p.extend(u16be(
        (2 * GRID_CENTER).div_ceil(u32::from(SECTOR_SIZE)) as u16
    ));
    p.push(48); // spangdh * 10
    p.extend(u16be(539)); // nsp1 * 100
    p.extend(u16be(40)); // nsp2 * 100
    p.extend(u16be(1400)); // nsp3 * 100
    p.extend(u16be(33)); // mamu * 1000
    p.extend(u16be(28)); // manu2 * 1000
    p.extend(u16be(430)); // cst * 1000
    p.push(PROTOCOL);
    p.push(physics::ANCHOR_SPACING as u8);
    p.extend(u16be(SERVER_NUMBER));
    p.extend(u24be(checked(radius.round(), MAX_RADIUS, "Radius")? as u32));
    debug_assert_eq!(p.len(), 30);
    Ok(p)
}
pub(crate) fn radius(radius: f64) -> Result<Vec<u8>, String> {
    let mut p = vec![b'z'];
    p.extend(u24be(checked(radius.round(), MAX_RADIUS, "Radius")? as u32));
    Ok(p)
}

/// Full snake on entry or a new life. Points run tail to head; the head-side
/// anchor is an msl step at a 16-bit angle from the reconstructed previous point.
pub(crate) fn appearance(
    id: u16,
    snake: &Snake,
    wanted: f64,
    profile: &PlayerAppearance,
) -> Result<Vec<u8>, String> {
    if snake.body.len() < 2 {
        return Err("Appearance needs at least two body anchors".into());
    }
    let (nickname, custom) = (&profile.nickname, &profile.custom_skin);
    if nickname.len() > 255 || custom.len() > 255 {
        return Err("Nickname and custom skin must fit a one-byte length".into());
    }
    let mut p = vec![b's'];
    p.extend(u16be(id));
    p.extend(u24be(angle_u24(snake.heading)));
    p.push(b'0' + direction(snake.heading, wanted));
    p.extend(u24be(angle_u24(wanted)));
    p.extend(u16be(speed_u16(snake.speed)?));
    p.extend(u24be(fam_u24(snake.fam)));
    p.push(profile.skin);
    p.extend(coord_u24(snake.head.x)?);
    p.extend(coord_u24(snake.head.y)?);
    p.push(nickname.len() as u8);
    p.extend(nickname);
    p.push(custom.len() as u8);
    p.extend(custom);
    p.push(0); // cosmetic
    let tail = *snake.body.last().unwrap();
    let x = coord_u24(tail.x)?;
    let y = coord_u24(tail.y)?;
    p.extend(x);
    p.extend(y);
    let decode = |b: [u8; 3]| f64::from(u32::from_be_bytes([0, b[0], b[1], b[2]])) / 5.;
    let mut last = Point {
        x: decode(x),
        y: decode(y),
    };
    let n = snake.body.len();
    for point in snake.body[1..n - 1].iter().rev() {
        let step = |value: f64, from: &mut f64| -> Result<u8, String> {
            let byte = checked(
                ((wire(value) - *from) * 2.).round() + 127.,
                255.,
                "Body step",
            )?;
            *from += (byte - 127.) / 2.;
            Ok(byte as u8)
        };
        let dx = step(point.x, &mut last.x)?;
        let dy = step(point.y, &mut last.y)?;
        p.extend([dx, dy]);
    }
    let front = snake.body[0];
    p.extend(u16be(angle_u16(
        (wire(front.y) - last.y).atan2(wire(front.x) - last.x),
    )));
    Ok(p)
}
/// Snake leaves this client's view; `died` marks a death rather than distance.
pub(crate) fn removal(id: u16, died: bool) -> Vec<u8> {
    let mut p = vec![b's'];
    p.extend(u16be(id));
    p.push(u8::from(died));
    p
}
/// Changed motion fields only. Own-snake forms omit the id. Forms carrying the
/// wanted heading also set the stock client's turn `direction`, which selects
/// between paired codes; no turn uses the first (an immediate snap to wanted).
pub(crate) fn motion(
    own: bool,
    id: u16,
    heading: Option<u8>,
    wanted: Option<u8>,
    speed: Option<u8>,
    direction: u8,
) -> Option<Vec<u8>> {
    let pick = |decreasing: u8, increasing: u8| {
        if direction == 2 {
            increasing
        } else {
            decreasing
        }
    };
    let (code, values): (u8, Vec<u8>) = match (heading, wanted, speed) {
        (None, None, None) => return None,
        (Some(a), Some(w), Some(s)) if own => (pick(b'd', b'7'), vec![a, w, s]),
        (Some(a), Some(w), Some(s)) => (pick(b'e', b'E'), vec![a, w, s]),
        (Some(a), None, Some(s)) => (b'e', vec![a, s]),
        (None, Some(w), Some(s)) => (pick(b'E', b'4'), vec![w, s]),
        (Some(a), Some(w), None) => (pick(b'3', b'5'), vec![a, w]),
        (Some(a), None, None) => (b'e', vec![a]),
        (None, Some(w), None) => (pick(b'E', b'4'), vec![w]),
        (None, None, Some(s)) => (b'3', vec![s]),
    };
    let mut p = with_id(code, own, id);
    p.extend(values);
    Some(p)
}
/// New front anchor an msl step from the previous one; growth carries fam.
pub(crate) fn anchor(own: bool, id: u16, angle: u16, fam: Option<u32>) -> Vec<u8> {
    let code = match (own, fam.is_some()) {
        (true, false) => b'G',
        (true, true) => b'N',
        (false, false) => b'g',
        (false, true) => b'n',
    };
    let mut p = with_id(code, own, id);
    p.extend(u16be(angle));
    if let Some(fam) = fam {
        p.extend(u24be(fam));
    }
    p
}
/// New front anchor at absolute integer coordinates.
pub(crate) fn anchor_at(
    own: bool,
    id: u16,
    angle: u16,
    point: Point,
    fam: Option<u32>,
) -> Result<Vec<u8>, String> {
    let mut p = with_id(if fam.is_some() { b'+' } else { b'=' }, own, id);
    p.extend(u16be(angle));
    p.extend(coord_u16(point.x)?);
    p.extend(coord_u16(point.y)?);
    if let Some(fam) = fam {
        p.extend(u24be(fam));
    }
    Ok(p)
}
/// One tail anchor removed, with the remaining fam.
pub(crate) fn tail(id: u16, fam: u32) -> Vec<u8> {
    let mut p = vec![b'r'];
    p.extend(u16be(id));
    p.extend(u24be(fam));
    p
}
pub(crate) fn fam(id: u16, fam: u32) -> Vec<u8> {
    let mut p = vec![b'h'];
    p.extend(u16be(id));
    p.extend(u24be(fam));
    p
}

/// Exact wire food key `(sector x, sector y, offset x, offset y)`.
pub(crate) fn food_key(point: Point) -> Result<[u8; 4], String> {
    let cell = |value: f64| -> Result<u32, String> {
        Ok(checked(
            (wire(value) * 256. / f64::from(SECTOR_SIZE)).round(),
            65535.,
            "Food position",
        )? as u32)
    };
    let (x, y) = (cell(point.x)?, cell(point.y)?);
    Ok([(x >> 8) as u8, (y >> 8) as u8, x as u8, y as u8])
}
pub(crate) fn food_size(size: f64) -> u8 {
    (size * 5.).round().clamp(0., 255.) as u8
}
/// Presentation only; the bot decoder deliberately omits color and birth kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FoodStyle {
    pub color: u8,
    pub size: u8,
    pub snake_food: bool,
}
/// Sector listing on subscription, split into frameable packets.
pub(crate) fn food_sector(sector: [u8; 2], foods: &[([u8; 2], FoodStyle)]) -> Vec<Vec<u8>> {
    let chunks: Vec<_> = if foods.is_empty() {
        vec![foods]
    } else {
        foods.chunks((MAX_PACKET - 3) / 4).collect()
    };
    chunks
        .into_iter()
        .map(|chunk| {
            let mut p = vec![b'F', sector[0], sector[1]];
            for &([rx, ry], style) in chunk {
                p.extend([style.color, rx, ry, style.size]);
            }
            p
        })
        .collect()
}
/// Explicit color/sector: no dependency on another connection's implicit state.
pub(crate) fn food_add(key: [u8; 4], style: FoodStyle) -> Vec<u8> {
    vec![
        if style.snake_food { b'b' } else { b'f' },
        key[0],
        key[1],
        key[2],
        key[3],
        style.color,
        style.size,
    ]
}
pub(crate) fn food_remove(key: [u8; 4], eater: Option<u16>) -> Vec<u8> {
    let mut p = vec![
        if eater.is_some() { b'<' } else { b'c' },
        key[0],
        key[1],
        key[2],
        key[3],
    ];
    if let Some(id) = eater {
        p.extend(u16be(id));
    }
    p
}
pub(crate) fn sector_clear(sector: [u8; 2]) -> Vec<u8> {
    vec![b'w', sector[0], sector[1]]
}
pub(crate) fn sector_enter(sector: [u8; 2]) -> Vec<u8> {
    vec![b'W', sector[0], sector[1]]
}

pub(crate) fn prey_id(prey: &Prey) -> u16 {
    prey.id as u16
}
/// Prey motion position on the wire's 3-unit grid.
pub(crate) fn prey_grid(point: Point) -> Result<[u16; 2], String> {
    let cell = |value: f64| {
        checked(((wire(value) - 1.) / 3.).round(), 65535., "Prey position").map(|v| v as u16)
    };
    Ok([cell(point.x)?, cell(point.y)?])
}
pub(crate) fn prey_appearance(prey: &Prey) -> Result<Vec<u8>, String> {
    let mut p = vec![b'y'];
    p.extend(u16be(prey_id(prey)));
    p.push(0); // color
    p.extend(coord_u24(prey.position.x)?);
    p.extend(coord_u24(prey.position.y)?);
    p.push(food_size(prey.size));
    p.push(b'0' + direction(prey.heading, prey.target_heading));
    p.extend(u24be(angle_u24(prey.target_heading)));
    p.extend(u24be(angle_u24(prey.heading)));
    p.extend(u16be(speed_u16(prey.speed)?));
    debug_assert_eq!(p.len(), 20);
    Ok(p)
}
/// Position, plus heading, wanted heading and speed when `full`.
pub(crate) fn prey_motion(prey: &Prey, full: bool) -> Result<Vec<u8>, String> {
    let mut p = vec![b'j'];
    p.extend(u16be(prey_id(prey)));
    for value in prey_grid(prey.position)? {
        p.extend(u16be(value));
    }
    if full {
        p.push(b'0' + direction(prey.heading, prey.target_heading));
        p.extend(u24be(angle_u24(prey.heading)));
        p.extend(u24be(angle_u24(prey.target_heading)));
        p.extend(u16be(speed_u16(prey.speed)?));
    }
    Ok(p)
}
pub(crate) fn prey_removal(id: u16) -> Vec<u8> {
    let mut p = vec![b'y'];
    p.extend(u16be(id));
    p
}

/// Score the stock client displays for `segments` and a wire fam, using its
/// `setMscps(430)` tables; the setup advertises `MAX_SEGMENTS` as `mscps`.
pub(crate) fn display_score(segments: usize, fam: u32) -> f64 {
    physics::score(segments, f64::from(fam) / U24)
}
/// Display-table index past which the stock client repeats its last entry.
const SCORE_LAST_INDEX: usize = physics::MAX_SEGMENTS + 2048;
/// Own score credit `R`: segments the stock client adds before scoring.
pub(crate) fn credit(credit: u8) -> Vec<u8> {
    vec![b'R', credit]
}
/// Byte credit whose stock display score is nearest `target`, the smaller on a
/// tie, following the source bridge's `segment_credit`. Mass already showing
/// at least `target` needs none.
pub(crate) fn segment_credit(target: f64, segments: usize, fam: u32) -> u8 {
    if !target.is_finite() {
        return 0;
    }
    let upper = SCORE_LAST_INDEX.saturating_sub(segments).min(255);
    let shown = |credit: usize| display_score(segments + credit, fam);
    if target <= shown(0) {
        return 0;
    }
    if target >= shown(upper) {
        return upper as u8;
    }
    (0..=upper)
        .min_by(|&a, &b| {
            (shown(a) - target)
                .abs()
                .total_cmp(&(shown(b) - target).abs())
                .then(a.cmp(&b))
        })
        .unwrap_or(0) as u8
}
/// Visible snake's kill counter (`k` in the stock client).
pub(crate) fn kills(id: u16, kills: usize) -> Vec<u8> {
    let mut p = vec![b'k'];
    p.extend(u16be(id));
    p.extend(u24be(kills.min(U24 as usize) as u32));
    p
}
/// One leaderboard row: wire mass the stock client scores with its tables,
/// a color byte it reduces modulo 9 and the name.
pub(crate) fn leaderboard_row(
    out: &mut Vec<u8>,
    segments: usize,
    fam: u32,
    color: u8,
    name: &[u8],
) -> Result<(), String> {
    if segments > 65535 || name.len() > 255 {
        return Err("Leaderboard row does not fit protocol-19 fields".into());
    }
    out.extend(u16be(segments as u16));
    out.extend(u24be(fam));
    out.extend([color, name.len() as u8]);
    out.extend(name);
    Ok(())
}
/// Leaderboard: own row position among `rows` (0 when absent), own rank and
/// living population, then the encoded top rows.
pub(crate) fn leaderboard(position: u8, rank: usize, count: usize, rows: &[u8]) -> Vec<u8> {
    let mut p = vec![b'l', position];
    p.extend(u16be(rank.min(65535) as u16));
    p.extend(u16be(count.min(65535) as u16));
    p.extend(rows);
    p
}
/// Record holder (`m`): 24-bit segments and fam, name, then the message bytes.
pub(crate) fn record(
    segments: usize,
    fam: u32,
    name: &[u8],
    message: &[u8],
) -> Result<Vec<u8>, String> {
    if segments > U24 as usize || name.len() > 255 {
        return Err("Record does not fit protocol-19 fields".into());
    }
    let mut p = vec![b'm'];
    p.extend(u24be(segments as u32));
    p.extend(u24be(fam));
    p.push(name.len() as u8);
    p.extend(name);
    p.extend(message);
    Ok(p)
}
/// Optional session-ID response: snake id, then an opaque 32-bit session.
pub(crate) fn session(id: u16, session: u32) -> Vec<u8> {
    let mut p = vec![b'S'];
    p.extend(u16be(id));
    p.extend(session.to_be_bytes());
    p
}

pub(crate) fn pong() -> Vec<u8> {
    vec![b'p']
}
pub(crate) fn game_over(record: bool) -> Vec<u8> {
    vec![b'v', u8::from(record)]
}
pub(crate) fn victory_accepted() -> Vec<u8> {
    vec![b'v', 2]
}

/// Full `M` snapshot: cells are written from the last index down, seven bits per
/// literal byte (most significant first) or skipped in zero runs.
pub(crate) fn minimap(map: &Minimap) -> Vec<u8> {
    let mut p = vec![b'M'];
    p.extend(u16be(map.side as u16));
    let mut remaining = map.cells.len();
    while remaining > 0 {
        let zeros = map.cells[..remaining]
            .iter()
            .rev()
            .take_while(|&&cell| cell == 0)
            .count();
        if zeros == remaining {
            break;
        }
        if zeros >= 7 {
            let mut left = zeros;
            while left >= 126 {
                let runs = (left / 126).min(255);
                p.extend([255, runs as u8]);
                left -= runs * 126;
            }
            if left > 0 {
                p.push(128 + left as u8);
            }
            remaining -= zeros;
            continue;
        }
        let mut byte = 0;
        for bit in (0..7).rev() {
            if remaining == 0 {
                break;
            }
            remaining -= 1;
            if map.cells[remaining] != 0 {
                byte |= 1 << bit;
            }
        }
        p.push(byte);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn food_packets_keep_color_origin_and_explicit_eater() {
        let ambient = FoodStyle {
            color: 7,
            size: 35,
            snake_food: false,
        };
        let snake = FoodStyle {
            color: 3,
            size: 68,
            snake_food: true,
        };
        assert_eq!(
            food_sector([108, 109], &[([20, 30], ambient), ([40, 50], snake)]),
            [vec![b'F', 108, 109, 7, 20, 30, 35, 3, 40, 50, 68]]
        );
        assert_eq!(
            food_add([108, 109, 20, 30], ambient),
            [b'f', 108, 109, 20, 30, 7, 35]
        );
        assert_eq!(
            food_add([108, 109, 40, 50], snake),
            [b'b', 108, 109, 40, 50, 3, 68]
        );
        assert_eq!(
            food_remove([108, 109, 20, 30], Some(1)),
            [b'<', 108, 109, 20, 30, 0, 1]
        );
        assert_eq!(
            food_remove([108, 109, 20, 30], None),
            [b'c', 108, 109, 20, 30]
        );
    }

    fn u24(p: &[u8], m: usize) -> f64 {
        f64::from(u32::from_be_bytes([0, p[m], p[m + 1], p[m + 2]]))
    }

    /// Mirror of `split` in tools/fidelity/slither_protocol.py.
    fn split(a: &[u8]) -> Vec<Vec<u8>> {
        let mut m = 2;
        if a[m] >= 32 {
            return vec![a[m..].to_vec()];
        }
        let mut out = Vec::new();
        while m < a.len() {
            let n = if a[m] < 32 {
                m += 2;
                usize::from(a[m - 2]) << 8 | usize::from(a[m - 1])
            } else {
                m += 1;
                usize::from(a[m - 1] - 32)
            };
            out.push(a[m..m + n].to_vec());
            m += n;
        }
        out
    }

    #[test]
    fn frames_follow_the_capture_splitter() {
        let one = frame(7, &[pong()]).unwrap();
        assert_eq!(one, [0, 7, b'p']);
        assert_eq!(split(&one), [pong()]);
        let packets = vec![removal(3, true), food_remove([1, 2, 3, 4], None), pong()];
        let many = frame(300, &packets).unwrap();
        assert_eq!(&many[..2], &300u16.to_be_bytes());
        assert_eq!(split(&many), packets);
        assert!(frame(0, &[]).is_err());
        assert!(frame(0, &[vec![b'F'; MAX_PACKET + 1], pong()]).is_err());
    }

    #[test]
    fn minimap_round_trips_through_the_client_decoder() {
        for (side, pattern) in [(1, 1), (7, 3), (32, 5), (40, 97), (512, 4093)] {
            let cells: Vec<u8> = (0..side * side)
                .map(|i| u8::from(i % pattern == 0 && i % 3 != 1))
                .collect();
            let map = Minimap {
                side,
                cells,
                age_seconds: 0.,
            };
            assert_eq!(Minimap::packet(None, &minimap(&map)).unwrap(), map);
        }
        let empty = Minimap {
            side: 32,
            cells: vec![0; 1024],
            age_seconds: 0.,
        };
        assert_eq!(minimap(&empty).len(), 3);
        assert_eq!(Minimap::packet(None, &minimap(&empty)).unwrap(), empty);
    }

    #[test]
    fn appearance_decodes_with_the_capture_layout() {
        let heading = 1.1_f64;
        let mut body = vec![Point { x: -10.3, y: 20.7 }];
        for step in 1..12 {
            let previous = body[step - 1];
            let angle = 0.3 * step as f64;
            body.push(Point {
                x: previous.x - angle.cos() * 30.,
                y: previous.y - angle.sin() * 30.,
            });
        }
        // Exact msl step from the second anchor, as crossed anchors produce.
        let angle = f64::from(angle_u16(0.7)) * TAU / 65536.;
        body[0] = Point {
            x: body[1].x + angle.cos() * 42.,
            y: body[1].y + angle.sin() * 42.,
        };
        let snake = Snake {
            id: "4".into(),
            head: Point { x: 3.3, y: -4.4 },
            body: body.clone(),
            heading,
            speed: 185.,
            radius: 14.5,
            length: 12.,
            segments: 12,
            fam: 0.25,
            score: 0.,
            kills: 0,
            alive: true,
        };
        let p = appearance(513, &snake, 2., &PlayerAppearance::default()).unwrap();
        // A native slot's empty profile keeps the original zero-length fields.
        assert_eq!(p[15], 0);
        assert_eq!(p[22..25], [0, 0, 0]);
        // Field offsets of snake_packet in slither_protocol.py.
        assert_eq!(p[0], b's');
        assert_eq!(u16::from_be_bytes([p[1], p[2]]), 513);
        assert!((u24(&p, 3) * TAU / U24 - heading).abs() < 1e-6);
        assert!((u24(&p, 7) * TAU / U24 - 2.).abs() < 1e-6);
        assert_eq!(f64::from(u16::from_be_bytes([p[10], p[11]])) / 1000., 5.92);
        assert!((u24(&p, 12) / U24 - 0.25).abs() < 1e-7);
        let grid = f64::from(GRID_CENTER);
        assert!((u24(&p, 16) / 5. - grid - 3.3).abs() <= 0.1 + 1e-9);
        assert!((u24(&p, 19) / 5. - grid + 4.4).abs() <= 0.1 + 1e-9);
        let mut m = 22;
        m += 1 + usize::from(p[m]);
        m += 1 + usize::from(p[m]);
        m += 1;
        let mut points: Vec<Point> = Vec::new();
        while m < p.len() {
            let last = points.last().copied().unwrap_or_default();
            if points.is_empty() {
                points.push(Point {
                    x: u24(&p, m) / 5.,
                    y: u24(&p, m + 3) / 5.,
                });
                m += 6;
            } else if m == p.len() - 2 {
                let a = f64::from(u16::from_be_bytes([p[m], p[m + 1]])) * TAU / 65536.;
                points.push(Point {
                    x: last.x + a.cos() * 42.,
                    y: last.y + a.sin() * 42.,
                });
                m += 2;
            } else {
                points.push(Point {
                    x: last.x + (f64::from(p[m]) - 127.) / 2.,
                    y: last.y + (f64::from(p[m + 1]) - 127.) / 2.,
                });
                m += 2;
            }
        }
        assert_eq!(points.len(), body.len());
        for (decoded, actual) in points.iter().rev().zip(&body) {
            assert!((decoded.x - grid - actual.x).abs() <= 0.35);
            assert!((decoded.y - grid - actual.y).abs() <= 0.35);
        }
    }

    #[test]
    fn appearance_profile_fills_skin_nickname_and_custom_skin() {
        let head = Point { x: 1., y: 2. };
        let snake = Snake {
            id: "5".into(),
            head,
            body: vec![head, Point { x: -41., y: 2. }],
            heading: 0.,
            speed: 185.,
            radius: 14.5,
            length: 2.,
            segments: 2,
            fam: 0.,
            score: 0.,
            kills: 0,
            alive: true,
        };
        let plain = appearance(5, &snake, 1., &PlayerAppearance::default()).unwrap();
        let profile = PlayerAppearance {
            nickname: b"Bot".to_vec(),
            skin: 9,
            custom_skin: vec![1, 2],
        };
        let mut expected = plain.clone();
        expected[15] = 9;
        expected.splice(22..24, [3, b'B', b'o', b't', 2, 1, 2]);
        assert_eq!(appearance(5, &snake, 1., &profile).unwrap(), expected);
        let long = PlayerAppearance {
            custom_skin: vec![0; 256],
            ..profile
        };
        assert!(appearance(5, &snake, 1., &long).is_err());
    }

    #[test]
    fn prey_packets_carry_the_signed_turn_direction() {
        for (target, expected) in [(1.2, b'2'), (0.8, b'1'), (1., b'0')] {
            let prey = Prey {
                id: 3,
                position: Point { x: 10., y: -20. },
                heading: 1.,
                target_heading: target,
                speed: 100.,
                size: 4.,
            };
            let appearance = prey_appearance(&prey).unwrap();
            assert_eq!((appearance[3], appearance[11]), (0, expected));
            assert_eq!(prey_motion(&prey, true).unwrap()[7], expected);
        }
    }

    #[test]
    fn simulator_food_grid_maps_exactly_to_wire_sectors() {
        for x in [-999.7, -0.2, 0., 0.4, 517.9, 31_000.] {
            let point = Point {
                x: physics::quantize_food(x),
                y: physics::quantize_food(-x),
            };
            let [sx, sy, rx, ry] = food_key(point).unwrap();
            let wire = |s: u8, r: u8| f64::from(s) * 300. + f64::from(r) * 300. / 256.;
            assert!((wire(sx, rx) - f64::from(GRID_CENTER) - point.x).abs() < 1e-9);
            assert!((wire(sy, ry) - f64::from(GRID_CENTER) - point.y).abs() < 1e-9);
        }
        assert!(food_key(Point { x: 50_000., y: 0. }).is_err());
    }

    #[test]
    fn motion_forms_match_decoder_lengths_and_turn_direction() {
        // (own, fields) -> (code turning down, code turning up, length) per
        // rotation() in slither_protocol.py and the stock direction pairs.
        let cases = [
            (false, [true, true, true], b'e', b'E', 6),
            (false, [true, false, true], b'e', b'e', 5),
            (false, [false, true, true], b'E', b'4', 5),
            (false, [true, true, false], b'3', b'5', 5),
            (false, [true, false, false], b'e', b'e', 4),
            (false, [false, true, false], b'E', b'4', 4),
            (false, [false, false, true], b'3', b'3', 4),
            (true, [true, true, true], b'd', b'7', 4),
            (true, [true, false, true], b'e', b'e', 3),
            (true, [false, true, true], b'E', b'4', 3),
            (true, [true, true, false], b'3', b'5', 3),
            (true, [true, false, false], b'e', b'e', 2),
            (true, [false, true, false], b'E', b'4', 2),
            (true, [false, false, true], b'3', b'3', 2),
        ];
        for (own, [a, w, s], down, up, len) in cases {
            for (direction, code) in [(0, down), (1, down), (2, up)] {
                let p = motion(
                    own,
                    9,
                    a.then_some(1),
                    w.then_some(2),
                    s.then_some(3),
                    direction,
                );
                let p = p.unwrap();
                assert_eq!((p[0], p.len()), (code, len));
                let values = &p[if own { 1 } else { 3 }..];
                let expected: Vec<u8> = [(a, 1), (w, 2), (s, 3)]
                    .into_iter()
                    .filter_map(|(sent, value)| sent.then_some(value))
                    .collect();
                assert_eq!(values, expected);
            }
        }
        assert!(motion(true, 9, None, None, None, 1).is_none());
        assert_eq!(direction(0., 0.1), 2);
        assert_eq!(direction(0., -0.1), 1);
        assert_eq!(direction(0.1, TAU - 0.1), 1);
        assert_eq!(direction(TAU - 0.1, 0.1), 2);
        assert_eq!(direction(1., 1.), 0);
        assert_eq!(anchor(true, 9, 7, None).len(), 3);
        assert_eq!(anchor(true, 9, 7, Some(1)).len(), 6);
        assert_eq!(anchor(false, 9, 7, None).len(), 5);
        assert_eq!(anchor(false, 9, 7, Some(1)).len(), 8);
        let at = Point { x: 1., y: 2. };
        assert_eq!(anchor_at(true, 9, 7, at, None).unwrap().len(), 7);
        assert_eq!(anchor_at(false, 9, 7, at, Some(1)).unwrap().len(), 12);
        assert_eq!(setup(1000.).unwrap().len(), 30);
        assert!(setup(MAX_RADIUS + 1.).is_err());
    }

    /// Stock-client `l` handler: position, rank, count, then sct/fam/color/name.
    #[test]
    fn hud_packets_follow_the_stock_handlers() {
        let mut rows = Vec::new();
        leaderboard_row(&mut rows, 20, fam_u24(0.5), 12, b"Ann").unwrap();
        leaderboard_row(&mut rows, 300, 0, 0, b"").unwrap();
        assert!(leaderboard_row(&mut rows.clone(), 70000, 0, 0, b"").is_err());
        let p = leaderboard(2, 291, 413, &rows);
        assert_eq!(p[..6], [b'l', 2, 1, 35, 1, 157]);
        assert_eq!(p[6..8], [0, 20]);
        assert!((u24(&p, 8) / U24 - 0.5).abs() < 1e-7);
        assert_eq!(p[11..16], [12, 3, b'A', b'n', b'n']);
        assert_eq!(p[16..], [1, 44, 0, 0, 0, 0, 0]);
        assert_eq!(leaderboard(0, 70000, 1, &[])[2..4], [255, 255]);

        // The stock display expression with its 430-entry tables.
        let table = |i: usize| (1. - i.min(429) as f64 / 430.).powf(2.25);
        let sum: f64 = (0..20).map(|i| 1. / table(i)).sum();
        let stock = ((sum + 0.5 / table(20) - 1.) * 15. - 5.).floor();
        assert_eq!(display_score(20, fam_u24(0.5)), stock);

        let m = record(431, fam_u24(1.), b"Bo", b"gg").unwrap();
        assert_eq!(
            m,
            [b'm', 0, 1, 175, 255, 255, 255, 2, b'B', b'o', b'g', b'g']
        );
        assert!(record(1 << 24, 0, b"", b"").is_err());
        assert_eq!(kills(258, 5), [b'k', 1, 2, 0, 0, 5]);
        assert_eq!(kills(1, usize::MAX)[3..], [255, 255, 255]);
        assert_eq!(session(3, 0x0102_0304), [b'S', 0, 3, 1, 2, 3, 4]);
    }

    #[test]
    fn credit_is_the_nearest_byte_of_the_stock_display() {
        let fam = fam_u24(0.25);
        let shown = |segments| display_score(segments, fam);
        assert_eq!(credit(7), [b'R', 7]);
        assert_eq!(segment_credit(shown(40), 40, fam), 0);
        assert_eq!(segment_credit(shown(40) - 100., 40, fam), 0);
        assert_eq!(segment_credit(f64::NAN, 40, fam), 0);
        assert_eq!(segment_credit(shown(43), 40, fam), 3);
        // Between two credits the nearer wins, the smaller on a tie.
        let near = shown(42) + (shown(43) - shown(42)) * 0.4;
        assert_eq!(segment_credit(near, 40, fam), 2);
        let tie = (shown(42) + shown(43)) / 2.;
        assert_eq!(segment_credit(tie, 40, fam), 2);
        // Beyond index 430 the stock tables repeat, so no credit adds score
        // there; like the bridge, a target the largest credit reaches takes it.
        let top = shown(430);
        assert_eq!(display_score(431, fam), top);
        assert_eq!(segment_credit(top + 1e6, 400, fam), 255);
        assert_eq!(segment_credit(top, 400, fam), 255);
        assert_eq!(segment_credit(shown(429), 400, fam), 29);
    }
}
