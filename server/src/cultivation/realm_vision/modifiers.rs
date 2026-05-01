use crate::schema::realm_vision::RealmVisionParamsV1;

pub type RealmVisionEnvModifier = super::planner::RealmVisionEnvModifier;
pub type RealmVisionStatusModifier = super::planner::RealmVisionStatusModifier;
pub const FLOOR_CLAMP_M: f64 = super::planner::FLOOR_CLAMP_M;

pub fn apply_status_modifiers(
    params: &mut RealmVisionParamsV1,
    modifiers: &[RealmVisionStatusModifier],
) {
    super::planner::apply_status_modifiers(params, modifiers);
}

pub fn apply_env_modifiers(
    params: &mut RealmVisionParamsV1,
    realm: crate::cultivation::components::Realm,
    modifiers: &[RealmVisionEnvModifier],
) {
    super::planner::apply_env_modifiers(params, realm, modifiers);
}
