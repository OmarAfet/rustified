use crate::error::{AuthError, Result, ResultExt};
use reqwest::Client;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use tracing::{debug, error, trace};

use super::constants::{XBL_AUTH_URL, XSTS_AUTH_URL};
use super::models::{
    XboxLiveProperties, XboxLiveRequest, XboxLiveResponse, XstsProperties, XstsRequest,
};

/// Get Xbox Live token using the Microsoft access token
pub async fn get_xbox_live_token(client: &Client, ms_token: &str) -> Result<(String, String)> {
    // Build the Xbox Live authentication request
    let xbl_request = XboxLiveRequest {
        properties: XboxLiveProperties {
            auth_method: "RPS".to_string(),
            site_name: "user.auth.xboxlive.com".to_string(),
            rps_ticket: format!("d={ms_token}"),
        },
        relying_party: "http://auth.xboxlive.com".to_string(),
        token_type: "JWT".to_string(),
    };

    debug!("Sending authentication request to Xbox Live: {XBL_AUTH_URL}");
    let response = client
        .post(XBL_AUTH_URL)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .json(&xbl_request)
        .send()
        .await
        .with_context(|| {
            "Failed to send request to Xbox Live authentication endpoint".to_string()
        })?;

    let status = response.status();
    debug!("Received response from Xbox Live with status: {status}");

    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|e| {
            error!("Failed to read error response body: {e}");
            "Unknown error".to_string()
        });
        error!("Xbox Live authentication failed with status {status}: {error_text}");
        return Err(AuthError::xbox_auth(format!(
            "Xbox Live authentication failed: {status} - {error_text}"
        ))
        .into());
    }

    let xbl_response: XboxLiveResponse = response
        .json()
        .await
        .with_context(|| "Failed to parse Xbox Live response as JSON".to_string())?;

    let user_hash = if let Some(info) = xbl_response.display_claims.xui.first() {
        info.uhs.clone()
    } else {
        error!("No Xbox User Hash found in response");
        return Err(AuthError::xbox_auth("No Xbox User Hash found in response").into());
    };

    debug!("Successfully retrieved Xbox Live token and user hash");
    trace!(
        "User hash: {}, Token length: {}",
        user_hash,
        xbl_response.token.len()
    );

    Ok((xbl_response.token, user_hash))
}

/// Get XSTS token using the Xbox Live token
pub async fn get_xsts_token(client: &Client, xbl_token: &str) -> Result<String> {
    // Build the XSTS authentication request
    let xsts_request = XstsRequest {
        properties: XstsProperties {
            sandbox_id: "RETAIL".to_string(),
            user_tokens: vec![xbl_token.to_string()],
        },
        relying_party: "rp://api.minecraftservices.com/".to_string(),
        token_type: "JWT".to_string(),
    };

    debug!("Sending XSTS authentication request to: {XSTS_AUTH_URL}");
    let response = client
        .post(XSTS_AUTH_URL)
        .header(CONTENT_TYPE, "application/json")
        .json(&xsts_request)
        .send()
        .await
        .with_context(|| "Failed to send request to XSTS authentication endpoint".to_string())?;

    let status = response.status();
    debug!("Received response from XSTS with status: {status}");

    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_else(|e| {
            error!("Failed to read error response body: {e}");
            "Unknown error".to_string()
        });

        // Special handling for common error codes
        if status.as_u16() == 401 {
            if error_text.contains("2148916233") {
                error!(
                    "XSTS authentication failed: Account belongs to a child (under 18) and requires adult approval"
                );
                return Err(AuthError::xbox_auth(
                    "Xbox Live account belongs to a child (under 18) and requires adult approval",
                )
                .into());
            } else if error_text.contains("2148916238") {
                error!(
                    "XSTS authentication failed: Account is from a country/region where Xbox Live is not available"
                );
                return Err(AuthError::xbox_auth(
                    "Xbox Live is not available in your country/region",
                )
                .into());
            }
        }

        error!("XSTS authentication failed with status {status}: {error_text}");
        return Err(AuthError::xbox_auth(format!(
            "XSTS authentication failed: {status} - {error_text}"
        ))
        .into());
    }

    let xsts_response: XboxLiveResponse = response
        .json()
        .await
        .with_context(|| "Failed to parse XSTS response as JSON".to_string())?;

    debug!("Successfully retrieved XSTS token");
    trace!("XSTS token length: {}", xsts_response.token.len());

    Ok(xsts_response.token)
}
