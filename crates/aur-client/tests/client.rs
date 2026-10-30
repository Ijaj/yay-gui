use std::time::Duration;

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};
use yaygui_aur_client::{AurClient, AurConfig, Error, SearchBy};

const SEARCH: &str = include_str!("fixtures/search_response.json");
const INFO: &str = include_str!("fixtures/info_response.json");
const ERROR: &str = include_str!("fixtures/error_response.json");

fn client(server: &MockServer, ttl_secs: u64) -> AurClient {
    AurClient::new(AurConfig {
        base_url: server.uri(),
        cache_ttl: Duration::from_secs(ttl_secs),
        min_request_interval: Duration::ZERO,
        ..AurConfig::default()
    })
    .unwrap()
}

fn json(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.to_owned(), "application/json")
}

#[tokio::test]
async fn search_parses_results_and_helpers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rpc/v5/search/foo"))
        .and(query_param("by", "name-desc"))
        .respond_with(json(SEARCH))
        .expect(1)
        .mount(&server)
        .await;
    let res = client(&server, 0)
        .search("foo", SearchBy::NameDesc)
        .await
        .unwrap();
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].name, "foo-bin");
    assert_eq!(res[0].num_votes, 42);
    assert!(!res[0].is_orphan() && !res[0].is_out_of_date());
    assert!(res[1].is_orphan() && res[1].is_out_of_date());
    assert_eq!(res[1].description, None);
    assert!(res[0].depends.is_empty());
}

#[tokio::test]
async fn search_url_encodes_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rpc/v5/search/a%20b%2Fc"))
        .and(query_param("by", "maintainer"))
        .respond_with(json(SEARCH))
        .expect(1)
        .mount(&server)
        .await;
    client(&server, 0)
        .search("a b/c", SearchBy::Maintainer)
        .await
        .unwrap();
}

#[tokio::test]
async fn short_query_rejected_without_request() {
    let server = MockServer::start().await;
    let err = client(&server, 0)
        .search("a", SearchBy::Name)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::QueryTooShort));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn info_parses_full_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rpc/v5/info"))
        .and(query_param("arg[]", "foo-bin"))
        .respond_with(json(INFO))
        .mount(&server)
        .await;
    let res = client(&server, 0)
        .info(&["foo-bin", "foo-bin"])
        .await
        .unwrap();
    assert_eq!(res.len(), 1);
    let p = &res[0];
    assert_eq!(p.depends, ["glibc", "gtk3"]);
    assert_eq!(p.make_depends, ["cargo"]);
    assert_eq!(p.opt_depends, ["bar: extra support"]);
    assert_eq!(p.check_depends, ["check"]);
    assert_eq!(p.co_maintainers, ["bob"]);
    assert_eq!(p.license, ["GPL-3.0-or-later"]);
    assert_eq!(p.submitter.as_deref(), Some("alice"));
    assert_eq!(
        p.url_path.as_deref(),
        Some("/cgit/aur.git/snapshot/foo-bin.tar.gz")
    );
}

/// Echoes a minimal package for every requested `arg[]`.
struct Echo;
impl Respond for Echo {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let results: Vec<serde_json::Value> = req
            .url
            .query_pairs()
            .filter(|(k, _)| k == "arg[]")
            .map(|(_, v)| serde_json::json!({"Name": v, "PackageBase": v, "Version": "1-1"}))
            .collect();
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "version": 5, "type": "multiinfo",
            "resultcount": results.len(), "results": results
        }))
    }
}

#[tokio::test]
async fn info_batches_450_names() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rpc/v5/info"))
        .respond_with(Echo)
        .mount(&server)
        .await;
    let names: Vec<String> = (0..450).map(|i| format!("pkg-{i}")).collect();
    let res = client(&server, 0).info(&names).await.unwrap();
    assert_eq!(res.len(), 450);
    assert_eq!(res[0].name, "pkg-0");
    assert_eq!(res[449].name, "pkg-449");
    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 3);
}

#[tokio::test]
async fn info_batches_by_url_length() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rpc/v5/info"))
        .respond_with(Echo)
        .mount(&server)
        .await;
    let names: Vec<String> = (0..100)
        .map(|i| format!("{}{i:03}", "x".repeat(97)))
        .collect();
    let res = client(&server, 0).info(&names).await.unwrap();
    assert_eq!(res.len(), 100);
    let reqs = server.received_requests().await.unwrap();
    assert!(reqs.len() >= 3);
    for r in reqs {
        assert!(
            r.url.as_str().len() <= 4400,
            "url too long: {}",
            r.url.as_str().len()
        );
    }
}

#[tokio::test]
async fn error_envelope_is_typed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(json(ERROR))
        .mount(&server)
        .await;
    let err = client(&server, 0)
        .search("foo", SearchBy::Name)
        .await
        .unwrap_err();
    match err {
        Error::Rpc(m) => assert_eq!(m, "Too many package results."),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn cache_hit_skips_network() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rpc/v5/info"))
        .respond_with(json(INFO))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rpc/v5/search/foo"))
        .respond_with(json(SEARCH))
        .mount(&server)
        .await;
    let c = client(&server, 60);

    c.info(&["foo-bin"]).await.unwrap();
    c.info(&["foo-bin"]).await.unwrap();
    c.search("foo", SearchBy::Name).await.unwrap();
    c.search("foo", SearchBy::Name).await.unwrap();
    c.search("foo", SearchBy::Keywords).await.unwrap(); // different `by`: miss
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn http_errors_are_mapped() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rpc/v5/search/five"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rpc/v5/search/many"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "30"))
        .mount(&server)
        .await;
    let c = client(&server, 0);
    assert!(matches!(
        c.search("five", SearchBy::Name).await,
        Err(Error::Status(503))
    ));
    match c.search("many", SearchBy::Name).await {
        Err(Error::RateLimited { retry_after }) => {
            assert_eq!(retry_after, Some(Duration::from_secs(30)));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn rejects_bad_base_url() {
    let r = AurClient::new(AurConfig {
        base_url: "nonsense".into(),
        ..AurConfig::default()
    });
    assert!(matches!(r, Err(Error::InvalidUrl(_))));
}
