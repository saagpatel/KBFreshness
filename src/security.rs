use reqwest::Url;
use std::net::IpAddr;

pub fn validate_outbound_url(url: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|_| "invalid URL format".to_string())?;

    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("only http/https URLs are allowed".to_string()),
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("embedded URL credentials are not allowed".to_string());
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| "URL host is missing".to_string())?;
    let host_lower = host.to_ascii_lowercase();

    if host_lower == "localhost"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
        || host_lower.ends_with(".internal")
    {
        return Err("local/internal hostnames are blocked".to_string());
    }

    let host_for_ip_parse = host.trim_start_matches('[').trim_end_matches(']');

    if let Ok(ip) = host_for_ip_parse.parse::<IpAddr>() {
        if is_disallowed_ip(ip) {
            return Err("private or local network addresses are blocked".to_string());
        }
    }

    Ok(())
}

fn is_disallowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_outbound_url;

    #[test]
    fn allows_public_http_and_https_urls() {
        assert!(validate_outbound_url("https://example.com/path").is_ok());
        assert!(validate_outbound_url("http://example.org").is_ok());
    }

    #[test]
    fn blocks_invalid_scheme_or_credentials() {
        assert!(validate_outbound_url("ftp://example.com/file").is_err());
        assert!(validate_outbound_url("https://user:pass@example.com").is_err());
    }

    #[test]
    fn blocks_local_and_private_targets() {
        assert!(validate_outbound_url("http://localhost:3000").is_err());
        assert!(validate_outbound_url("http://127.0.0.1:8080").is_err());
        assert!(validate_outbound_url("http://10.0.0.5").is_err());
        assert!(validate_outbound_url("http://[::1]").is_err());
    }
}
