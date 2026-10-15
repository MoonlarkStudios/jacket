//! Actionable browser-session failures shared by access tests and normal jobs.
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessProblem {
    pub code: &'static str,
    pub message: &'static str,
    pub guidance: &'static str,
}

pub fn problem(detail: &str) -> Option<AccessProblem> {
    let detail = detail.to_ascii_lowercase();
    let contains = |patterns: &[&str]| patterns.iter().any(|pattern| detail.contains(pattern));
    let (code, message, guidance) = if contains(&[
        "cookies are no longer valid",
        "session has expired",
        "cookies have expired",
        "cookies are expired",
    ]) {
        ("expired", "Your YouTube session has expired.", "Open the selected browser profile, sign in to YouTube again, then return to Session settings and test access. Jacket reads the session again on your next attempt.")
    } else if contains(&[
        "dpapi",
        "failed to decrypt",
        "cannot decrypt",
        "keychain",
        "find-generic-password",
        "no key found",
    ]) {
        ("decryption", "Jacket could not unlock this browser session.", "Allow the browser’s Keychain request if macOS shows one, then test again. If it still fails, use another signed-in browser such as Firefox. Keep browser encryption enabled.")
    } else if contains(&[
        "permission denied",
        "operation not permitted",
        "access is denied",
        "access denied",
        "blocked access to this browser session",
    ]) {
        ("permission", "Your operating system blocked access to this browser session.", "Check Jacket’s browser-data permissions, then test again. You can also choose another signed-in browser. Public access remains available.")
    } else if contains(&[
        "database is locked",
        "could not copy",
        "being used by another process",
        "database locked",
        "cookie database is busy",
    ]) {
        ("locked", "The browser’s cookie database is busy.", "Close the selected browser, then test access again. Jacket will not close your browser for you.")
    } else if contains(&[
        "could not find",
        "cookies database not found",
        "profile is no longer available",
        "choose your browser profile",
    ]) && contains(&["profile", "cookie"])
    {
        ("profileMissing", "The selected browser profile is no longer available.", "Open Session settings, refresh the profile list, and choose the profile where you are signed in. Jacket will not switch profiles automatically.")
    } else if contains(&[
        "http error 429",
        "too many requests",
        "request rate limit",
        "youtube is limiting requests",
    ]) {
        ("rateLimited", "YouTube is limiting requests from this connection.", "Wait before trying again. Reconnecting or changing profiles is not a reliable fix for a rate limit.")
    } else if contains(&[
        "sign in",
        "sign-in",
        "login required",
        "authentication required",
        "private video",
        "private playlist",
        "members-only",
        "confirm your age",
        "age-restricted",
        "session could not access the requested content",
    ]) {
        ("signIn", "This browser session could not access the requested content.", "Open the link in the selected browser profile and confirm that you are signed in and can play it there. Then return to Session settings and test again; choose another profile if the account is wrong.")
    } else {
        return None;
    };
    Some(AccessProblem {
        code,
        message,
        guidance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_distinguishes_session_problems_from_provider_failures() {
        for (message, code) in [
            (
                "The provided YouTube account cookies are no longer valid",
                "expired",
            ),
            (
                "WARNING: cannot decrypt v10 cookies: no key found",
                "decryption",
            ),
            ("Permission denied: Cookies.binarycookies", "permission"),
            ("Could not copy Chrome cookie database", "locked"),
            ("could not find firefox cookies database", "profileMissing"),
            ("ERROR: Private video. Sign in", "signIn"),
            ("HTTP Error 429: Too Many Requests", "rateLimited"),
        ] {
            assert_eq!(problem(message).unwrap().code, code);
        }
        assert!(problem("HTTP Error 403: Forbidden").is_none());
        assert!(problem("Network connection timed out").is_none());
    }
}
