use super::*;
use crate::config::KvmConfig;
use crate::kvm::KvmController;

/// KVM switch status response
#[derive(Serialize)]
pub struct KvmStatusResponse {
    pub available: bool,
    pub current_channel: u8,
}

/// Get KVM switch status
pub async fn kvm_status(State(state): State<Arc<AppState>>) -> Result<Json<KvmStatusResponse>> {
    let kvm_guard = state.kvm.read().await;

    match kvm_guard.as_ref() {
        Some(kvm) => {
            let channel = kvm.current_channel().await;
            Ok(Json(KvmStatusResponse {
                available: kvm.is_configured(),
                current_channel: channel,
            }))
        }
        None => Ok(Json(KvmStatusResponse {
            available: false,
            current_channel: 0,
        })),
    }
}

/// KVM switch channel request
#[derive(Deserialize)]
pub struct KvmSwitchRequest {
    pub channel: u8, // 1–4
}

/// Switch KVM channel
pub async fn kvm_switch(
    State(state): State<Arc<AppState>>,
    Json(req): Json<KvmSwitchRequest>,
) -> Result<Json<LoginResponse>> {
    let kvm_guard = state.kvm.read().await;
    let kvm = kvm_guard
        .as_ref()
        .ok_or_else(|| AppError::Internal("KVM switch controller not initialized".to_string()))?;

    let channel = kvm.switch_channel(req.channel).await?;

    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("Switched to KVM channel {}", channel)),
    }))
}

/// Get KVM configuration
pub async fn get_kvm_config(State(state): State<Arc<AppState>>) -> Json<KvmConfig> {
    Json(state.config.get().kvm.clone())
}

/// Update KVM configuration and restart the controller
pub async fn update_kvm_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<KvmConfig>,
) -> Result<Json<KvmConfig>> {
    let mut new_config = req;
    new_config.normalize();

    state
        .config
        .update(|config| {
            config.kvm = new_config.clone();
        })
        .await?;

    // Tear down the existing controller.
    {
        let old = state.kvm.write().await.take();
        if let Some(controller) = old {
            let _ = controller.shutdown().await;
        }
    }

    // Build a new controller if enabled.
    if new_config.enabled {
        let controller = KvmController::new(new_config.to_controller_config());
        match controller.init().await {
            Ok(()) => {
                *state.kvm.write().await = Some(controller);
            }
            Err(e) => {
                tracing::warn!("Failed to reinitialize KVM switch controller: {}", e);
            }
        }
    }

    Ok(Json(new_config))
}
