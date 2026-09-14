#![allow(dead_code, unused_imports)]
use bong_server::network::audio_trigger::*;
use valence::prelude::*;

/// 契约 pin：心跳 recipe 必须是 `player_local`（→ 收件人 `Single(player)`），
/// 因为 stop 侧写死了 `AudioRecipient::Single`。若有人把 attenuation 改成广播类，
/// play 会到别人客户端、stop 却只回自己 → 别人耳朵里的心跳永生。
#[test]
fn low_hp_heartbeat_recipe_is_player_local_so_single_recipient_stop_matches() {
    let registry = bong_server::audio::SoundRecipeRegistry::load_default()
        .expect("默认 audio recipe 应能加载");
    let recipe = registry
        .get("heartbeat_low_hp")
        .expect("heartbeat_low_hp recipe 应存在");
    assert!(
            recipe.loop_cfg.is_some(),
            "期望 heartbeat_low_hp 仍是 loop recipe 因为整套 play/stop 配对治法就是为 loop 设计的；实际没有 loop 段"
        );
    assert_eq!(
        recipe.attenuation,
        bong_server::schema::audio::AudioAttenuation::PlayerLocal,
        "期望 heartbeat_low_hp 保持 player_local 因为 stop 侧按 Single(player) 定向；\
             改成广播类会让别人客户端收到 play 却收不到 stop（心跳在他们耳里永生）；实际 {:?}",
        recipe.attenuation,
    );
}

/// **接线门禁（事件侧）**：三条签名链读的事件必须由**各自模块的生产 `register`** 装进 World，
/// 否则 cast 侧 `world.send_event` 会静默丢弃 → 实机零签名音。
///
/// 测试调生产 register 而非自己 `add_event`：从生产 register 里删掉 `add_event` 即撞红
/// （已变异验证）。
#[test]
fn production_module_registers_install_signature_cast_events() {
    use bong_server::combat::dugu_v2::ReverseTriggeredEvent;
    use bong_server::combat::tuike_v2::FalseSkinSheddedEvent;
    use bong_server::combat::zhenmai_v2::ZhenmaiSkillCastEvent;

    let mut app = App::new();
    bong_server::combat::zhenmai_v2::register(&mut app);
    assert!(
        app.world()
            .contains_resource::<Events<ZhenmaiSkillCastEvent>>(),
        "zhenmai_v2::register 必须 add_event::<ZhenmaiSkillCastEvent>()——\
             缺它则 emit_skill_feedback 的 send_event 被静默丢弃，实机零招式音"
    );

    let mut app = App::new();
    bong_server::combat::dugu_v2::register(&mut app);
    assert!(
        app.world()
            .contains_resource::<Events<ReverseTriggeredEvent>>(),
        "dugu_v2::register 必须 add_event::<ReverseTriggeredEvent>()——缺它则倒蚀签名音无事件可读"
    );

    let mut app = App::new();
    bong_server::combat::tuike_v2::register(&mut app);
    assert!(
        app.world()
            .contains_resource::<Events<FalseSkinSheddedEvent>>(),
        "tuike_v2::register 必须 add_event::<FalseSkinSheddedEvent>()——\
             缺它则主动 / 被动蜕壳签名音都无事件可读"
    );
}
