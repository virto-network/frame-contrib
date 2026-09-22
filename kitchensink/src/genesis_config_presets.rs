//! Genesis configuration presets.

use crate::{BalancesConfig, RuntimeGenesisConfig, UNITS};
use alloc::{vec, vec::Vec};
use serde_json::Value;
use sp_genesis_builder::PresetId;
use sp_keyring::Sr25519Keyring;

/// The `development` preset: funds the well-known development accounts.
pub fn development_config_genesis() -> Value {
    frame_support::build_struct_json_patch!(RuntimeGenesisConfig {
        balances: BalancesConfig {
            balances: Sr25519Keyring::iter()
                .map(|a| (a.to_account_id(), 1_000_000 * UNITS))
                .collect::<Vec<_>>(),
        },
    })
}

/// Returns the preset with the given `id`, if any.
pub fn get_preset(id: &PresetId) -> Option<Vec<u8>> {
    let patch = match id.as_ref() {
        sp_genesis_builder::DEV_RUNTIME_PRESET => development_config_genesis(),
        _ => return None,
    };
    Some(
        serde_json::to_string(&patch)
            .expect("serialization to json is expected to work; qed")
            .into_bytes(),
    )
}

/// The available presets.
pub fn preset_names() -> Vec<PresetId> {
    vec![PresetId::from(sp_genesis_builder::DEV_RUNTIME_PRESET)]
}
