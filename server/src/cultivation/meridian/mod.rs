//! plan-meridian-severed-v1 · 经脉永久 SEVERED 通用底盘
//!
//! `severed` 子模块提供 `MeridianSeveredPermanent` component、`MeridianSeveredEvent`、
//! `SeveredSource` 7 类来源、`check_meridian_dependencies` 招式依赖经脉强约束检查、
//! `try_acupoint_repair` 接经术接口（plan-yidao-v1 占位）以及 `apply_severed_event_system`
//! 把 event 写入 component 的运行时桥。

pub mod severed;
