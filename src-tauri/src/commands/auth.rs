use crate::auth::{self, AuthSession, AuthUser};

#[tauri::command]
pub async fn auth_sign_up(email: String, password: String) -> Result<AuthSession, String> {
    auth::sign_up(&email, password).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn auth_sign_in(email: String, password: String) -> Result<AuthSession, String> {
    auth::sign_in(&email, password).await.map_err(|e| e.to_string())
}

/// `Ok(None)` means the saved session is no longer valid; `Err` means it couldn't be checked.
#[tauri::command]
pub async fn auth_restore_session(token: String) -> Result<Option<AuthUser>, String> {
    auth::restore(&token).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn auth_sign_out(token: String) -> Result<(), String> {
    auth::sign_out(&token).await.map_err(|e| e.to_string())
}
