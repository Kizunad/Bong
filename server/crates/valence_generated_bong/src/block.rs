#![allow(clippy::all)] // TODO: block build script creates many warnings.

use std::fmt;
use std::fmt::Display;
use std::iter::FusedIterator;

use valence_ident::{ident, Ident};

use crate::item::ItemKind;

include!(concat!(env!("OUT_DIR"), "/block.rs"));

impl fmt::Debug for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_block_state(*self, f)
    }
}

impl Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_block_state(*self, f)
    }
}

fn fmt_block_state(bs: BlockState, f: &mut fmt::Formatter) -> fmt::Result {
    let kind = bs.to_kind();

    write!(f, "{}", kind.to_str())?;

    let props = kind.props();

    if !props.is_empty() {
        let mut list = f.debug_list();
        for &p in kind.props() {
            struct KeyVal<'a>(&'a str, &'a str);

            impl<'a> fmt::Debug for KeyVal<'a> {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, "{}={}", self.0, self.1)
                }
            }

            list.entry(&KeyVal(p.to_str(), bs.get(p).unwrap().to_str()));
        }
        list.finish()
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_set_consistency() {
        for kind in BlockKind::ALL {
            let block = kind.to_state();

            for &prop in kind.props() {
                let new_block = block.set(prop, block.get(prop).unwrap());
                assert_eq!(new_block, block);
            }
        }
    }

    #[test]
    fn blockstate_to_wall() {
        assert_eq!(BlockState::STONE.wall_block_id(), None);
        assert_eq!(
            BlockState::OAK_SIGN.wall_block_id(),
            Some(BlockState::OAK_WALL_SIGN)
        );
        assert_eq!(
            BlockState::GREEN_BANNER.wall_block_id(),
            Some(BlockState::GREEN_WALL_BANNER)
        );
        assert_ne!(
            BlockState::GREEN_BANNER.wall_block_id(),
            Some(BlockState::GREEN_BANNER)
        );
    }

    #[test]
    fn bong_blocks_extend_vanilla_registry() {
        assert_eq!(BlockState::max_raw(), 24140);
        assert_eq!(BlockState::from_raw(24135), Some(BlockState::BONG_ZHENFA_NODE));
        assert_eq!(BlockState::from_raw(24141), None);
        assert_eq!(BlockState::BONG_ZHENFA_NODE.to_kind(), BlockKind::BongZhenfaNode);
        assert_eq!(BlockKind::from_str("bong_zhenfa_node"), Some(BlockKind::BongZhenfaNode));
        assert_eq!(BlockKind::BongZhenfaNode.translation_key(), "block.bong.zhenfa_node");
        assert_eq!(BlockState::BONG_ZHENFA_NODE.luminance(), 3);
        assert!(!BlockState::BONG_ZHENFA_NODE.is_opaque());

        let line_x = BlockState::BONG_ZHENFA_LINE.set(PropName::Axis, PropValue::X);
        let line_z = BlockState::BONG_ZHENFA_LINE.set(PropName::Axis, PropValue::Z);
        assert_eq!(line_x.to_raw(), 24136);
        assert_eq!(line_z.to_raw(), 24138);

        assert_eq!(
            BlockState::BONG_ZHENFA_EYE.get(PropName::Charged),
            Some(PropValue::True)
        );
        assert_eq!(
            BlockState::BONG_ZHENFA_EYE
                .set(PropName::Charged, PropValue::False)
                .to_raw(),
            24140
        );
    }
}
