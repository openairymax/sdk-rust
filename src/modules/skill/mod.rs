// SPDX-FileCopyrightText: 2025-2026 SPHARX Ltd.
// SPDX-License-Identifier: AGPL-3.0-or-later OR Apache-2.0

// AgentRT Rust SDK - 技能管理模块
// Version: 0.1.1
// Last updated: 2026-03-24
//
// 提供技能的加载、执行、查询等功能。
// 对应 Go SDK: modules/skill/manager.go

mod manager;

pub use manager::SkillManager;
