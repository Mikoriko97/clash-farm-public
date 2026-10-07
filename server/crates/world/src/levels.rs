//! Level thresholds: level n+1 is reached at `120 * n^1.7` XP (docs/ECONOMY_LAB.md §4.1; 0019),
//! precomputed as integers so runtime code never touches floats.

const THRESHOLDS: [i64; 40] = [
    120, 389, 776, 1266, 1851, 2523, 3279, 4115, 5027, 6014, 7072, 8199, 9394, 10656, 11982, 13371, 14823,
    16335, 17908, 19540, 21230, 22977, 24780, 26640, 28554, 30523, 32546, 34621, 36750, 38930, 41161, 43444,
    45777, 48160, 50593, 53075, 55606, 58185, 60812, 63486,
];

/// Level for a given XP total; new islands start at level 1 with 0 XP.
pub fn level_for_xp(xp: i64) -> i32 {
    let reached = THRESHOLDS.iter().filter(|&&t| xp >= t).count();
    i32::try_from(reached).unwrap_or(i32::MAX).saturating_add(1)
}

/// XP still needed for the next level; `None` at the cap.
pub fn xp_to_next(xp: i64) -> Option<i64> {
    THRESHOLDS.iter().find(|&&t| xp < t).map(|t| t - xp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_thresholds() {
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(119), 1);
        assert_eq!(level_for_xp(120), 2);
        assert_eq!(level_for_xp(389), 3);
        assert_eq!(level_for_xp(5_027), 10);
        assert_eq!(level_for_xp(1_000_000), 41);
        assert_eq!(xp_to_next(50), Some(70));
        assert_eq!(xp_to_next(1_000_000), None);
    }
}
