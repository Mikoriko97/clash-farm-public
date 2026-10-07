use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorldError {
    /// Also used for foreign resources: existence is never confirmed (docs/API.md).
    #[error("not found")]
    NotFound,
    #[error("plot is not in the required state")]
    WrongState,
    #[error("crop is not ready yet")]
    NotReady,
    #[error("not enough items in inventory")]
    InsufficientItems,
    #[error("insufficient credits")]
    InsufficientFunds,
    #[error("daily cap reached")]
    DailyCap,
    #[error("item cannot be used here")]
    InvalidItem,
    #[error("quantity out of range")]
    InvalidQty,
    /// Decor placement (0009): rotation step, footprint overlap, count limit.
    #[error("rotation must be a multiple of 15 degrees")]
    InvalidRotation,
    #[error("overlaps another object")]
    Overlap,
    #[error("decor limit reached")]
    DecorLimit,
    /// Island expansion (0010): unknown side, or the side already has four strips.
    #[error("side must be n, e, s or w")]
    InvalidSide,
    #[error("this side is fully expanded")]
    ExpandMax,
    /// Lightning asked for outside a storm slot.
    #[error("no storm right now")]
    NoStorm,
    #[error("plot already fertilized")]
    AlreadyFertilized,
    /// Placement (0006): outside the island top, cell occupied, level limit, bed too big.
    #[error("cell is outside the island")]
    OutOfBounds,
    #[error("cell already has a plot")]
    CellTaken,
    #[error("plot limit for this level reached")]
    PlotLimit,
    #[error("bed would exceed the size limit")]
    BedTooLarge,
    /// Cleared-bush spots of the island are used up (0023).
    #[error("cleared spots limit reached")]
    ClearingLimit,
    /// Character creation (0025, ADR-025): name shape (with the reason for the player),
    /// reserved/obscene name, name already taken, the account already has its character,
    /// look outside the colour tables.
    #[error("invalid nickname: {0}")]
    InvalidNickname(&'static str),
    #[error("nickname is not allowed")]
    NicknameNotAllowed,
    #[error("nickname is taken")]
    NicknameTaken,
    #[error("character already created")]
    CharacterExists,
    #[error("invalid look")]
    InvalidLook,
    /// Restyle (0029, ADR-038): no character yet, the same look again, no ticket owned.
    #[error("no character yet")]
    NoCharacter,
    #[error("same look")]
    SameLook,
    #[error("no restyle ticket")]
    NoRestyleTicket,
    /// Rename (0030, ADR-039): the name is already this account's; no ticket owned.
    #[error("same nickname")]
    SameNickname,
    #[error("no rename ticket")]
    NoRenameTicket,
    /// Resource node is used up and still regrowing (0007).
    #[error("resource is depleted")]
    Depleted,
    /// Exploration (0014/0015): presence puts the character too far, second visit, second claim.
    #[error("too far")]
    TooFar,
    #[error("already discovered")]
    AlreadyDiscovered,
    #[error("already claimed")]
    AlreadyClaimed,
    /// Exclusive items need reputation (docs/ECONOMY.md §3) or a purchase (week 3).
    #[error("exclusive item locked")]
    ExclusiveLocked,
    /// The item or action needs this island level (0019, docs/ECONOMY_LAB.md §4).
    #[error("requires island level {0}")]
    LevelLocked(i32),
    #[error("ledger error: {0}")]
    Ledger(cf_ledger::LedgerError),
    #[error("database error")]
    Db(#[from] sqlx::Error),
}

impl From<cf_ledger::LedgerError> for WorldError {
    fn from(err: cf_ledger::LedgerError) -> Self {
        match err {
            cf_ledger::LedgerError::InsufficientFunds => Self::InsufficientFunds,
            cf_ledger::LedgerError::Db(e) => Self::Db(e),
            other => Self::Ledger(other),
        }
    }
}
