//! Explicit prefix transition model for the resumable Z80.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndexRegister { Ix, Iy }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrefixTransition { ContinueM1 { active_prefix: u8 }, BaseOpcode, CbOpcode, EdOpcode, IndexedOpcode { index: IndexRegister }, IndexedCb { index: IndexRegister } }
pub(crate) const fn classify(previous: Option<u8>, next: u8) -> PrefixTransition {
    match previous {
        None => match next { 0xCB | 0xED | 0xDD | 0xFD => PrefixTransition::ContinueM1 { active_prefix: next }, _ => PrefixTransition::BaseOpcode },
        Some(0xDD | 0xFD) => match next {
            0xDD | 0xFD | 0xED => PrefixTransition::ContinueM1 { active_prefix: next },
            0xCB => PrefixTransition::IndexedCb { index: if matches!(previous, Some(0xDD)) { IndexRegister::Ix } else { IndexRegister::Iy } },
            _ => PrefixTransition::IndexedOpcode { index: if matches!(previous, Some(0xDD)) { IndexRegister::Ix } else { IndexRegister::Iy } },
        },
        Some(0xCB) => PrefixTransition::CbOpcode,
        Some(0xED) => PrefixTransition::EdOpcode,
        Some(_) => PrefixTransition::BaseOpcode,
    }
}
