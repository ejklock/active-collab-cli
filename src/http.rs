#![allow(dead_code)]

use anyhow::Result;
use reqwest::header::{HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::redirect;
use std::time::Duration;

const ACCEPT_JSON: &str = "application/json";
const TOKEN_HEADER: &str = "x-angie-authapitoken";

#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
}

impl Http {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .timeout(Duration::from_secs(30))
            .redirect(redirect::Policy::none())
            .build()?;
        Ok(Http { client })
    }

    /// Returns Some((header_name, header_value)) when the request URL's host
    /// matches the instance host — the token is never attached to a foreign host.
    pub fn host_gated_token_header(
        url: &str,
        instance_base_url: &str,
        token: &str,
    ) -> Option<(HeaderName, HeaderValue)> {
        let req_host = extract_host(url)?;
        let inst_host = extract_host(instance_base_url)?;
        if req_host.eq_ignore_ascii_case(&inst_host) {
            let name = HeaderName::from_static(TOKEN_HEADER);
            let value = HeaderValue::from_str(token).ok()?;
            Some((name, value))
        } else {
            None
        }
    }

    /// Authenticated GET. Returns Ok((status, body)) for any HTTP response
    /// (including 4xx/5xx). Only transport failures are Err.
    pub async fn authed_get(
        &self,
        url: &str,
        instance_base_url: &str,
        token: &str,
    ) -> Result<(u16, bytes::Bytes)> {
        let mut builder = self.client.get(url).header(ACCEPT, ACCEPT_JSON);

        if let Some((name, value)) = Self::host_gated_token_header(url, instance_base_url, token) {
            builder = builder.header(name, value);
        }

        let resp = builder.send().await?;
        let status = resp.status().as_u16();
        let body = resp.bytes().await?;
        Ok((status, body))
    }

    /// Authenticated POST with JSON body. Returns Ok((status, body)) for any
    /// HTTP response. Only transport failures are Err.
    pub async fn authed_post(
        &self,
        url: &str,
        instance_base_url: &str,
        token: &str,
        body: &serde_json::Value,
    ) -> Result<(u16, bytes::Bytes)> {
        let mut builder = self
            .client
            .post(url)
            .header(ACCEPT, ACCEPT_JSON)
            .header(CONTENT_TYPE, "application/json")
            .json(body);

        if let Some((name, value)) = Self::host_gated_token_header(url, instance_base_url, token) {
            builder = builder.header(name, value);
        }

        let resp = builder.send().await?;
        let status = resp.status().as_u16();
        let bytes = resp.bytes().await?;
        Ok((status, bytes))
    }

    /// Authenticated PUT with JSON body. Returns Ok((status, body)) for any
    /// HTTP response. Only transport failures are Err.
    pub async fn authed_put(
        &self,
        url: &str,
        instance_base_url: &str,
        token: &str,
        body: &serde_json::Value,
    ) -> Result<(u16, bytes::Bytes)> {
        let mut builder = self
            .client
            .put(url)
            .header(ACCEPT, ACCEPT_JSON)
            .header(CONTENT_TYPE, "application/json")
            .json(body);

        if let Some((name, value)) = Self::host_gated_token_header(url, instance_base_url, token) {
            builder = builder.header(name, value);
        }

        let resp = builder.send().await?;
        let status = resp.status().as_u16();
        let bytes = resp.bytes().await?;
        Ok((status, bytes))
    }

    /// Authenticated DELETE. Returns Ok((status, body)) for any HTTP response.
    /// Only transport failures are Err.
    pub async fn authed_delete(
        &self,
        url: &str,
        instance_base_url: &str,
        token: &str,
    ) -> Result<(u16, bytes::Bytes)> {
        let mut builder = self.client.delete(url).header(ACCEPT, ACCEPT_JSON);

        if let Some((name, value)) = Self::host_gated_token_header(url, instance_base_url, token) {
            builder = builder.header(name, value);
        }

        let resp = builder.send().await?;
        let status = resp.status().as_u16();
        let bytes = resp.bytes().await?;
        Ok((status, bytes))
    }

    /// Unauthenticated POST with JSON body. Returns Ok((status, body)) for
    /// any HTTP response. Only transport failures are Err.
    pub async fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<(u16, bytes::Bytes)> {
        let resp = self
            .client
            .post(url)
            .header(ACCEPT, ACCEPT_JSON)
            .header(CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .await?;
        let status = resp.status().as_u16();
        let bytes = resp.bytes().await?;
        Ok((status, bytes))
    }
}

fn extract_host(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    parsed.host_str().map(|h| h.to_lowercase())
}

#[cfg(test)]
#[path = "../tests/unit/http.rs"]
mod tests;
