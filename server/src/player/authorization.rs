use std::sync::Arc;

use valence::prelude::{bevy_ecs, Resource};

/// 中立的 operator 授权接口；命令模块提供实现，业务系统只依赖这个接口。
pub trait OperatorAuthorization: Send + Sync + 'static {
    fn allows_operator(&self, username: &str) -> bool;
}

#[derive(Resource, Clone)]
pub struct AuthorizationProvider {
    authorization: Arc<dyn OperatorAuthorization>,
}

impl AuthorizationProvider {
    pub fn new<T>(authorization: T) -> Self
    where
        T: OperatorAuthorization,
    {
        Self {
            authorization: Arc::new(authorization),
        }
    }

    pub fn allows_operator(&self, username: &str) -> bool {
        self.authorization.allows_operator(username)
    }

    #[cfg(test)]
    pub fn allow_user(username: impl Into<String>) -> Self {
        Self::new(StaticOperatorAuthorization {
            username: username.into(),
        })
    }
}

#[cfg(test)]
#[derive(Clone)]
struct StaticOperatorAuthorization {
    username: String,
}

#[cfg(test)]
impl OperatorAuthorization for StaticOperatorAuthorization {
    fn allows_operator(&self, username: &str) -> bool {
        self.username == username
    }
}
