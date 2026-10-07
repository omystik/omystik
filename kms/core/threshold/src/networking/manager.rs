// whole made for ØMYSTIK

use super::{health_check::HealthCheckSession, NetworkMode, Networking};
use crate::execution::runtime::party::{Role, RoleAssignment};
use crate::session_id::SessionId;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

#[async_trait]
pub trait NetworkingManager: Send + Sync {
    async fn make_healthcheck_session(
        &self,
        context_id: SessionId,
        role_assignment: &RoleAssignment,
        my_role: Role,
    ) -> anyhow::Result<HealthCheckSession>;

    async fn make_network_session(
        &self,
        session_id: SessionId,
        context_id: SessionId,
        role_assignment: &RoleAssignment,
        my_role: Role,
        network_mode: NetworkMode,
    ) -> anyhow::Result<Arc<dyn Networking + Send + Sync + 'static>>;
}

// Blanket impl so Arc<RwLock<GrpcNetworkingManager>> can be used as Arc<dyn NetworkingManager>
#[async_trait]
impl<T> NetworkingManager for RwLock<T>
where
    T: NetworkingManager + Send + Sync,
{
    async fn make_healthcheck_session(
        &self,
        context_id: SessionId,
        role_assignment: &RoleAssignment,
        my_role: Role,
    ) -> anyhow::Result<HealthCheckSession> {
        let inner = self.read().await;
        inner
            .make_healthcheck_session(context_id, role_assignment, my_role)
            .await
    }

    async fn make_network_session(
        &self,
        session_id: SessionId,
        context_id: SessionId,
        role_assignment: &RoleAssignment,
        my_role: Role,
        network_mode: NetworkMode,
    ) -> anyhow::Result<Arc<dyn Networking + Send + Sync + 'static>> {
        let inner = self.read().await;
        inner
            .make_network_session(
                session_id,
                context_id,
                role_assignment,
                my_role,
                network_mode,
            )
            .await
    }
}
