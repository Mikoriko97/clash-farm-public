//! Character creation (0025, ADR-025): the nickname and look a player picks once at the first
//! login. One character per account (a retry with the same key returns the first answer), names
//! are unique across lookalikes and case even under a parallel race, impersonating or obscene
//! names and out-of-table looks are refused before anything is stored, and the live name check
//! is capped per day. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, get, login, post, Session};
use serde_json::{json, Value};
use uuid::Uuid;

fn look() -> Value {
    json!({ "skin": 1, "shirt": 2, "pants": 3, "shoes": 4, "hair_color": 5, "hair": 2, "face": 1, "beard": 3, "hat": 4, "glasses": 1, "outfit": 3, "outfit_color": 6, "body": 1, "build": 2 })
}

/// A name nobody used in an earlier run: hex digits cannot spell anything the filter dislikes.
fn unique(prefix: &str) -> String {
    format!("{prefix}{}", &Uuid::new_v4().simple().to_string()[..8])
}

async fn create(app: &axum::Router, s: &Session, nick: &str) -> (StatusCode, Value) {
    post(
        app,
        s,
        "/me/character",
        Some(json!({ "nickname": nick, "look": look() })),
        None,
    )
    .await
}

async fn check(app: &axum::Router, s: &Session, nick: &str) -> (StatusCode, Value) {
    get(app, s, &format!("/me/nickname-check?nick={nick}")).await
}

async fn me(app: &axum::Router, s: &Session) -> Value {
    get(app, s, "/me").await.1["user"].clone()
}

#[tokio::test]
async fn test_character_is_created_once_and_shows_in_me() {
    let (app, pool) = app().await;
    let s = login(&app).await;
    let before = me(&app, &s).await;
    assert!(
        before["nickname"].is_null() && before["look"].is_null(),
        "{before}"
    );
    let name = unique("Tester");
    let (status, free) = get(&app, &s, &format!("/me/nickname-check?nick={name}")).await;
    assert_eq!(
        (status, &free["available"]),
        (StatusCode::OK, &json!(true)),
        "{free}"
    );

    let body = json!({ "nickname": name, "look": look() });
    let key = "6f0f5b3e-2a4c-4c39-9a52-3a0d6c1f0a11";
    let (status, first) = post(&app, &s, "/me/character", Some(body.clone()), Some(key)).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["nickname"], name);
    assert_eq!(first["look"], look());
    // A retry after a lost answer is the same answer, not "already created".
    let (status, again) = post(&app, &s, "/me/character", Some(body), Some(key)).await;
    assert_eq!((status, &again), (StatusCode::OK, &first));
    // A second, deliberate attempt cannot swap the name.
    let (status, second) = create(&app, &s, &unique("Other")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{second}");
    assert_eq!(second["error"]["code"], "character_exists");

    let after = me(&app, &s).await;
    assert_eq!(after["nickname"], name);
    assert_eq!(after["look"], look());
    let key: String = sqlx::query_scalar("SELECT nickname_key FROM cf.users WHERE id = $1")
        .bind(Uuid::parse_str(&s.user_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(key, cf_world::nickname::key_of(&name));
}

#[tokio::test]
async fn test_bad_and_impersonating_names_are_refused_and_nothing_is_stored() {
    let (app, _) = app().await;
    let s = login(&app).await;
    for nick in [
        "ab",
        "has space",
        "Тарас",
        "<b>x</b>",
        "tom-cat",
        "_lead",
        "trail_",
        "a__b",
        "abcdefghijklmnopq",
    ] {
        let (status, body) = create(&app, &s, nick).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{nick}: {body}");
        assert_eq!(body["error"]["code"], "invalid_nickname", "{nick}");
    }
    for nick in [
        "Admin",
        "Adm1n",
        "ClashSupport1",
        "Moderator",
        "Farmer",
        "Sh1theads",
        "ClashDev",
        "Adrnin",
        "xModeratorx",
    ] {
        let (status, body) = create(&app, &s, nick).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{nick}: {body}");
        assert_eq!(body["error"]["code"], "nickname_not_allowed", "{nick}");
    }
    let user = me(&app, &s).await;
    assert!(
        user["nickname"].is_null() && user["look"].is_null(),
        "still no character: {user}"
    );
}

#[tokio::test]
async fn test_lookalike_and_case_variants_of_a_taken_name_are_taken() {
    let (app, _) = app().await;
    let (a, b) = (login(&app).await, login(&app).await);
    let tail = &Uuid::new_v4().simple().to_string()[..8];
    let (status, body) = create(&app, &a, &format!("Cool{tail}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for twin in [
        format!("COOL{tail}"),
        format!("C00l{tail}"),
        format!("Coo1{tail}"),
        format!("cool_{tail}"),
    ] {
        let (status, body) = create(&app, &b, &twin).await;
        assert_eq!(status, StatusCode::CONFLICT, "{twin}: {body}");
        assert_eq!(body["error"]["code"], "nickname_taken", "{twin}");
        let (_, check) = get(&app, &b, &format!("/me/nickname-check?nick={twin}")).await;
        assert_eq!(check["reason"], "taken", "{twin}: {check}");
    }
    // B lost every round and can still choose a free name.
    let (status, _) = create(&app, &b, &unique("Free")).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_parallel_claims_of_one_name_have_one_winner() {
    let (app, _) = app().await;
    let (a, b) = (login(&app).await, login(&app).await);
    let name = unique("Race");
    let (ra, rb) = tokio::join!(create(&app, &a, &name), create(&app, &b, &name));
    let mut codes = [ra.0, rb.0];
    codes.sort();
    assert_eq!(codes, [StatusCode::OK, StatusCode::CONFLICT], "{ra:?} {rb:?}");
    let loser = if ra.0 == StatusCode::CONFLICT {
        &ra.1
    } else {
        &rb.1
    };
    assert_eq!(loser["error"]["code"], "nickname_taken");
}

#[tokio::test]
async fn test_look_must_be_inside_the_tables() {
    let (app, _) = app().await;
    let s = login(&app).await;
    let name = unique("Looky");
    for look in [
        json!({ "skin": 8, "shirt": 0, "pants": 0, "shoes": 0 }),
        json!({ "skin": 0, "shirt": 12, "pants": 0, "shoes": 0 }),
        json!({ "skin": 0, "shirt": 0, "pants": 12, "shoes": 0 }),
        json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 6 }),
    ] {
        let body = json!({ "nickname": name, "look": look });
        let (status, err) = post(&app, &s, "/me/character", Some(body), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{look}: {err}");
        assert_eq!(err["error"]["code"], "invalid_look");
    }
    // The parts of stage 3 have tables too.
    for (field, size) in [
        ("hair_color", 12),
        ("hair", 10),
        ("face", 5),
        ("beard", 6),
        ("hat", 8),
        ("glasses", 3),
        ("outfit", 9),
        ("outfit_color", 12),
        ("body", 2),
        ("build", 3),
    ] {
        let mut bad = look();
        bad[field] = json!(size);
        let body = json!({ "nickname": name, "look": bad });
        let (status, err) = post(&app, &s, "/me/character", Some(body), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{field}: {err}");
        assert_eq!(err["error"]["code"], "invalid_look", "{field}");
    }
    // Negative, oversized and unknown fields never reach the world layer.
    for look in [
        json!({ "skin": -1, "shirt": 0, "pants": 0, "shoes": 0 }),
        json!({ "skin": 300, "shirt": 0, "pants": 0, "shoes": 0 }),
        json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "body": -1 }),
        json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "body": 300 }),
        json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "body": 1.5 }),
        json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "body": null }),
        json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "build": -1 }),
        json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "build": 300 }),
        json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "build": null }),
        json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "build": 1.5 }),
        json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "build": "1" }),
        json!({ "skin": 0, "shirt": 0, "pants": 0, "shoes": 0, "wings": 1 }),
        json!({ "skin": 0, "shirt": 0, "pants": 0 }),
    ] {
        let body = json!({ "nickname": name, "look": look });
        let (status, _) = post(&app, &s, "/me/character", Some(body), None).await;
        assert!(status.is_client_error(), "{look}: {status}");
    }
    assert!(me(&app, &s).await["nickname"].is_null(), "nothing was stored");
}

#[tokio::test]
async fn test_name_check_explains_and_is_capped_per_day() {
    let (app, pool) = app().await;
    let s = login(&app).await;
    let (_, short) = check(&app, &s, "ab").await;
    assert_eq!(
        (&short["available"], &short["reason"]),
        (&json!(false), &json!("invalid")),
        "{short}"
    );
    assert!(!short["message"].as_str().unwrap().is_empty());
    let (_, rude) = check(&app, &s, "Admin").await;
    assert_eq!(rude["reason"], "not_allowed", "{rude}");
    // The day's budget is spent: no more probing, however the name looks.
    sqlx::query(
        "INSERT INTO cf.rate_events (user_id, action, day, count) VALUES ($1, 'nickname_check', (now() AT TIME ZONE 'UTC')::date, 200)
         ON CONFLICT (user_id, action, day) DO UPDATE SET count = 200",
    )
    .bind(Uuid::parse_str(&s.user_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let (status, capped) = check(&app, &s, &unique("Probe")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{capped}");
    assert_eq!(capped["error"]["code"], "daily_cap");
}

#[tokio::test]
async fn test_database_refuses_what_the_code_would_have_refused() {
    let (app, pool) = app().await;
    let (a, b) = (login(&app).await, login(&app).await);
    let (ua, ub) = (
        Uuid::parse_str(&a.user_id).unwrap(),
        Uuid::parse_str(&b.user_id).unwrap(),
    );
    let set = |id: Uuid, nick: Option<String>, key: Option<String>, look: Option<Value>| {
        let pool = pool.clone();
        async move {
            sqlx::query("UPDATE cf.users SET nickname = $2, nickname_key = $3, look = $4 WHERE id = $1")
                .bind(id)
                .bind(nick)
                .bind(key)
                .bind(look)
                .execute(&pool)
                .await
        }
    };
    // Shape: spaces, non-Latin letters, a leading digit, a doubled underscore.
    for bad in ["a b c", "Тарас", "1abc", "a__b", "ab"] {
        let r = set(ua, Some(bad.to_owned()), Some("k".to_owned()), Some(look())).await;
        assert!(r.is_err(), "{bad} must not be storable");
    }
    // The three columns exist together or not at all.
    let name = unique("Db");
    assert!(set(ua, Some(name.clone()), None, Some(look())).await.is_err());
    assert!(set(ua, Some(name.clone()), Some("k".to_owned()), None)
        .await
        .is_err());
    assert!(set(ua, None, Some("k".to_owned()), None).await.is_err());
    assert!(
        set(ua, Some(name.clone()), Some("k".to_owned()), Some(json!([1, 2])))
            .await
            .is_err()
    );
    // One key, one owner.
    let key = cf_world::nickname::key_of(&name);
    assert!(set(ua, Some(name.clone()), Some(key.clone()), Some(look()))
        .await
        .is_ok());
    assert!(set(ub, Some(format!("{name}_")), Some(key), Some(look()))
        .await
        .is_err());
    let twin = format!("x{name}");
    assert!(set(
        ub,
        Some(twin.clone()),
        Some(cf_world::nickname::key_of(&twin)),
        Some(look())
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn test_one_account_racing_two_names_keeps_only_one() {
    let (app, _) = app().await;
    let s = login(&app).await;
    let (first, second) = (unique("RaceA"), unique("RaceB"));
    let (ra, rb) = tokio::join!(create(&app, &s, &first), create(&app, &s, &second));
    let mut codes = [ra.0, rb.0];
    codes.sort();
    assert_eq!(codes, [StatusCode::OK, StatusCode::CONFLICT], "{ra:?} {rb:?}");
    let (winner, loser) = if ra.0 == StatusCode::OK {
        (&first, &rb.1)
    } else {
        (&second, &ra.1)
    };
    assert_eq!(loser["error"]["code"], "character_exists");
    assert_eq!(
        &me(&app, &s).await["nickname"],
        winner,
        "the loser did not overwrite the winner"
    );
}

/// Since the paid rename (0030, ADR-039) a player with a character checks names too (same daily
/// cap): another capitalisation of its own name is free for it, taken for everyone else.
#[tokio::test]
async fn test_an_account_with_a_character_checks_names_for_a_rename() {
    let (app, _) = app().await;
    let (s, other) = (login(&app).await, login(&app).await);
    let name = unique("Owner");
    let (status, _) = create(&app, &s, &name).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = check(&app, &s, &unique("Probe")).await;
    assert_eq!(
        (status, body["available"].clone()),
        (StatusCode::OK, json!(true)),
        "{body}"
    );
    let (_, own) = check(&app, &s, &name.to_lowercase()).await;
    assert_eq!(own["available"], true, "{own}");
    let (_, theirs) = check(&app, &other, &name.to_lowercase()).await;
    assert_eq!(
        (theirs["available"].clone(), theirs["reason"].clone()),
        (json!(false), json!("taken"))
    );
}

#[tokio::test]
async fn test_a_taken_name_costs_a_claim_and_a_success_gives_it_back() {
    let (app, pool) = app().await;
    let (a, b) = (login(&app).await, login(&app).await);
    let claims = |s: &Session| {
        let (pool, id) = (pool.clone(), Uuid::parse_str(&s.user_id).unwrap());
        async move {
            sqlx::query_scalar::<_, i32>(
                "SELECT coalesce(sum(count), 0)::int FROM cf.rate_events
                 WHERE user_id = $1 AND action = 'nickname_claim' AND day = (now() AT TIME ZONE 'UTC')::date",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let name = unique("Claim");
    assert_eq!(create(&app, &a, &name).await.0, StatusCode::OK);
    assert_eq!(claims(&a).await, 0, "a success is free");
    assert_eq!(create(&app, &b, &name).await.0, StatusCode::CONFLICT);
    assert_eq!(create(&app, &b, &name).await.0, StatusCode::CONFLICT);
    assert_eq!(claims(&b).await, 2, "every taken-name answer costs one");
    sqlx::query("UPDATE cf.rate_events SET count = 30 WHERE user_id = $1 AND action = 'nickname_claim'")
        .bind(Uuid::parse_str(&b.user_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = create(&app, &b, &unique("Later")).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["error"]["code"], "daily_cap");
}

#[tokio::test]
async fn test_a_client_that_predates_the_parts_still_creates_a_bald_male_character() {
    let (app, _) = app().await;
    let s = login(&app).await;
    let name = unique("Old");
    let old = json!({ "skin": 1, "shirt": 2, "pants": 3, "shoes": 4 });
    let (status, body) = post(
        &app,
        &s,
        "/me/character",
        Some(json!({ "nickname": name, "look": old })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let expect = json!({ "skin": 1, "shirt": 2, "pants": 3, "shoes": 4, "hair_color": 0, "hair": 0, "face": 0, "beard": 0, "hat": 0, "glasses": 0, "outfit": 0, "outfit_color": 0, "body": 0, "build": 0 });
    assert_eq!(body["look"], expect);
    assert_eq!(me(&app, &s).await["look"], expect);
}
