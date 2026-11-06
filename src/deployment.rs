use anyhow::{Result, bail};
use axum::http::{HeaderMap, StatusCode, Uri};
use std::net::IpAddr;

#[derive(Clone, Default)]
pub struct Deployment {
    secure: bool,
    trusted: Vec<(IpAddr, u8)>,
}
impl Deployment {
    pub fn new<'a>(
        external_url: Option<&str>,
        proxies: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self> {
        let mut deployment = Self::default();
        if let Some(url) = external_url {
            let uri: Uri = url.parse()?;
            if !matches!(uri.scheme_str(), Some("http" | "https"))
                || uri.host().is_none()
                || uri.authority().is_some_and(|a| a.as_str().contains('@'))
            {
                bail!("External URL must be an absolute HTTP or HTTPS URL without credentials");
            }
            deployment.secure = uri.scheme_str() == Some("https");
        }
        for proxy in proxies {
            let (ip, prefix) = match proxy.split_once('/') {
                Some((ip, prefix)) => (ip.parse::<IpAddr>()?, Some(prefix.parse::<u8>()?)),
                None => (proxy.parse::<IpAddr>()?, None),
            };
            let bits = if ip.is_ipv4() { 32 } else { 128 };
            let prefix = prefix.unwrap_or(bits);
            if prefix > bits {
                bail!("Invalid trusted proxy prefix");
            }
            deployment.trusted.push((ip, prefix));
        }
        Ok(deployment)
    }
    fn trusts(&self, ip: IpAddr) -> bool {
        self.trusted
            .iter()
            .any(|(network, prefix)| match (*network, ip) {
                (IpAddr::V4(a), IpAddr::V4(b)) => {
                    *prefix == 0
                        || (u32::from(a) >> (32 - prefix)) == (u32::from(b) >> (32 - prefix))
                }
                (IpAddr::V6(a), IpAddr::V6(b)) => {
                    *prefix == 0
                        || (u128::from(a) >> (128 - prefix)) == (u128::from(b) >> (128 - prefix))
                }
                _ => false,
            })
    }
    pub(crate) fn source(
        &self,
        peer: Option<IpAddr>,
        headers: &HeaderMap,
    ) -> Result<String, StatusCode> {
        let Some(mut source) = peer else {
            return Ok("unknown".into());
        };
        if !self.trusts(source) {
            return Ok(source.to_string());
        }
        let mut chain = Vec::new();
        for value in headers.get_all("x-forwarded-for") {
            for address in value
                .to_str()
                .map_err(|_| StatusCode::BAD_REQUEST)?
                .split(',')
            {
                chain.push(
                    address
                        .trim()
                        .parse::<IpAddr>()
                        .map_err(|_| StatusCode::BAD_REQUEST)?,
                );
            }
        }
        for address in chain.into_iter().rev() {
            if !self.trusts(source) {
                break;
            }
            source = address;
        }
        Ok(source.to_string())
    }
    pub(crate) fn cookie_suffix(&self) -> &'static str {
        if self.secure { "; Secure" } else { "" }
    }
}
