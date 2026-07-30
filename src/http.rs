#![allow(dead_code)]

pub const HTTP_UNAUTHORIZED: u16 = 401;

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

    /// Returns Some((header_name, header_value)) when the request URL's origin
    /// (scheme + host + port) matches the instance origin — the token is never
    /// attached to a request whose scheme, host, or port differs from the
    /// configured instance, even when the host alone matches.
    pub fn origin_gated_token_header(
        url: &str,
        instance_base_url: &str,
        token: &str,
    ) -> Option<(HeaderName, HeaderValue)> {
        let req_origin = origin_of(url)?;
        let inst_origin = origin_of(instance_base_url)?;
        if !req_origin.0.eq_ignore_ascii_case(&inst_origin.0) {
            return None;
        }
        if !req_origin.1.eq_ignore_ascii_case(&inst_origin.1) {
            return None;
        }
        if req_origin.2 != inst_origin.2 {
            return None;
        }
        let name = HeaderName::from_static(TOKEN_HEADER);
        let value = HeaderValue::from_str(token).ok()?;
        Some((name, value))
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

        if let Some((name, value)) = Self::origin_gated_token_header(url, instance_base_url, token)
        {
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

        if let Some((name, value)) = Self::origin_gated_token_header(url, instance_base_url, token)
        {
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

        if let Some((name, value)) = Self::origin_gated_token_header(url, instance_base_url, token)
        {
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

        if let Some((name, value)) = Self::origin_gated_token_header(url, instance_base_url, token)
        {
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

fn origin_of(url: &str) -> Option<(String, String, Option<u16>)> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_lowercase();
    let scheme = parsed.scheme().to_lowercase();
    let port = parsed.port_or_known_default();
    Some((scheme, host, port))
}

#[cfg(test)]
#[path = "../tests/unit/http.rs"]
mod tests;
