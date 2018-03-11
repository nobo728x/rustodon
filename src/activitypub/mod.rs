use serde::Serialize;
use serde_json::{self, Value};
use rocket::http::{Accept, ContentType, MediaType, Status};
use rocket::request::{self, FromRequest, Request};
use rocket::response::{self, Content, Responder};
use db::models::Account;

/// Newtype for JSON which represents JSON-LD ActivityStreams2 objects.
///
/// Implements `Responder`, so we can return this from Rocket routes
/// and have Content-Type and friends be handled ✨automagically✨.
pub struct ActivityStreams<T = Value>(pub T);
impl<T> Responder<'static> for ActivityStreams<T>
where
    T: Serialize,
{
    fn respond_to(self, req: &Request) -> response::Result<'static> {
        serde_json::to_string(&self.0)
            .map(|string| {
                let ap_json = ContentType::new("application", "activity+json");

                Content(ap_json, string).respond_to(req).unwrap()
            })
            .map_err(|e| {
                // TODO: logging (what happens if the Value won't serialize?)
                // the code i cribbed this from did some internal Rocket thing.
                Status::InternalServerError
            })
    }
}

/// A Rocket guard which forwards to the next handler unless the `Accept` header
/// is an ActivityStreams media type.
pub struct ActivityGuard();
impl<'a, 'r> FromRequest<'a, 'r> for ActivityGuard {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<ActivityGuard, ()> {
        use rocket::Outcome;

        if request.accept().map(is_as).unwrap_or(false) {
            Outcome::Success(ActivityGuard())
        } else {
            Outcome::Forward(())
        }
    }
}

/// Helper used in [`ActivityGuard`]; returns true if `accept` is an ActivityStreams-compatible
/// media type.
///
/// [`ActivityGuard`]: ./struct.ActivityGuard.html
fn is_as(accept: &Accept) -> bool {
    let media_type = accept.preferred().media_type();

    // TODO: clean this up/make these const, if MediaType::new ever becomes a const fn
    let ap_json = MediaType::new("application", "activity+json");
    let ap_json_ld = MediaType::with_params(
        "application",
        "ld+json",
        ("profile", "https://www.w3.org/ns/activitystreams"),
    );

    media_type.exact_eq(&ap_json) || media_type.exact_eq(&ap_json_ld)
}

/// Trait implemented by structs which can serialize to
/// ActivityPub-compliant ActivityStreams2 JSON-LD.
pub trait AsActivityPub {
    fn as_activitypub(&self) -> ActivityStreams;
}

impl AsActivityPub for Account {
    fn as_activitypub(&self) -> ActivityStreams<serde_json::Value> {
        ActivityStreams(json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Person",
            "id": self.get_uri(),

            "inbox": self.get_inbox_endpoint(),
            "outbox": self.get_outbox_endpoint(),

            "following": self.get_following_endpoint(),
            "followers": self.get_followers_endpoint(),

            "preferredUsername": self.username,
            "name": self.display_name.as_ref().map(String::as_str).unwrap_or(""),
            "summary": self.summary.as_ref().map(String::as_str).unwrap_or("<p></p>"),
        }))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn identifies_ap_requests() {
        use std::str::FromStr;

        let accept_json = Accept::from_str("application/activity+json").unwrap();
        let accept_json_ld = Accept::from_str(
            "application/ld+json; profile=\"https://www.w3.org/ns/activitystreams\"",
        ).unwrap();

        assert!(is_as(&accept_json_ld));
        assert!(is_as(&accept_json));
    }
}
