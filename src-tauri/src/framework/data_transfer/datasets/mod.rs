//! 框架自有的数据集适配（凭证、收藏与最近使用）
//!
//! 这里只放**框架自己拥有**的数据；插件数据（SSH 档案等）由插件在自己的装配阶段登记适配器。
//! 装配入口 `register_all` 由 `framework::register` 调用一次（幂等，重复调用无副作用）。

pub(crate) mod preferences;
pub(crate) mod vault;

/// 登记框架自有的适配器
pub(crate) fn register_all() {
    vault::register();
    preferences::register();
}
