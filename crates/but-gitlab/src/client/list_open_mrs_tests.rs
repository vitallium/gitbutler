use super::{GitLabClient, HttpStatusError, MAX_OPEN_MR_PAGES};
use crate::GitLabProjectId;
use but_secret::Sensitive;
use std::io::{ErrorKind, Read as _, Write as _};
use std::net::TcpListener;
use std::time::{Duration, Instant};

struct MockResponse {
    status: reqwest::StatusCode,
    body: String,
    next_page: Option<String>,
}

fn mr_json(iid: i64, source_branch: &str) -> String {
    format!(
        r#"{{"web_url":"https://gitlab.example/group/repo/-/merge_requests/{iid}","iid":{iid},"title":"MR {iid}","description":null,"author":null,"labels":[],"draft":false,"source_branch":"{source_branch}","target_branch":"main","sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","merge_commit_sha":null,"squash_commit_sha":null,"created_at":null,"updated_at":null,"merged_at":null,"closed_at":null,"project_id":1,"source_project_id":1,"target_project_id":1}}"#
    )
}

fn mock_client(
    responses: Vec<MockResponse>,
) -> (GitLabClient, std::thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for expected in responses {
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err)
                        if err.kind() == ErrorKind::WouldBlock && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(err) => panic!("expected list_open_mrs request: {err}"),
                }
            };
            stream.set_nonblocking(false).unwrap();

            let mut request = Vec::new();
            let mut chunk = [0; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).unwrap();
                assert_ne!(read, 0, "request should include complete HTTP headers");
                request.extend_from_slice(&chunk[..read]);
            }
            let request = String::from_utf8(request).unwrap();
            let request_line = request.lines().next().unwrap().to_owned();
            requests.push(request_line);

            let reason = expected.status.canonical_reason().unwrap_or("Unknown");
            let next_page = expected
                .next_page
                .map(|page| format!("x-next-page: {page}\r\n"))
                .unwrap_or_default();
            write!(
                stream,
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{next_page}\r\n{}",
                expected.status.as_u16(),
                reason,
                expected.body.len(),
                expected.body
            )
            .unwrap();
        }
        requests
    });
    let client = GitLabClient::new_with_host_override(
        &Sensitive("test-token".to_string()),
        &format!("http://{addr}"),
    )
    .unwrap();
    (client, server)
}

fn request_path(request_line: &str) -> &str {
    request_line.split_whitespace().nth(1).unwrap()
}

fn query_pairs(request_line: &str) -> Vec<(&str, &str)> {
    let path = request_path(request_line);
    let Some((_, query)) = path.split_once('?') else {
        return Vec::new();
    };
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .collect()
}

fn has_query(request_line: &str, key: &str, value: &str) -> bool {
    query_pairs(request_line)
        .iter()
        .any(|(k, v)| *k == key && *v == value)
}

#[tokio::test(flavor = "current_thread")]
async fn source_branch_filter_is_sent_and_does_not_list_every_open_mr() {
    let (client, server) = mock_client(vec![MockResponse {
        status: reqwest::StatusCode::OK,
        body: format!("[{}]", mr_json(7, "feature")),
        next_page: None,
    }]);

    let mrs = client
        .list_open_mrs_for_source_branch(GitLabProjectId::new("group", "repo"), "feature")
        .await
        .expect("source_branch list should succeed");
    assert_eq!(
        mrs.iter().map(|mr| mr.iid).collect::<Vec<_>>(),
        vec![7],
        "the matching MR must be returned"
    );

    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 1, "one source_branch query is one request");
    assert!(
        has_query(&requests[0], "state", "opened"),
        "source_branch list must request state=opened: {}",
        requests[0]
    );
    assert!(
        has_query(&requests[0], "source_branch", "feature"),
        "auto-detect must filter by source_branch: {}",
        requests[0]
    );
    assert!(
        !has_query(&requests[0], "page", "1"),
        "a single-branch lookup must not walk offset pages: {}",
        requests[0]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn concatenates_open_mr_pages_and_requests_per_page_100() {
    let (client, server) = mock_client(vec![
        MockResponse {
            status: reqwest::StatusCode::OK,
            body: format!("[{}]", mr_json(1, "feature")),
            next_page: Some("2".into()),
        },
        MockResponse {
            status: reqwest::StatusCode::OK,
            body: format!("[{}]", mr_json(101, "feature")),
            next_page: None,
        },
    ]);

    let mrs = client
        .list_open_mrs(GitLabProjectId::new("group", "repo"))
        .await
        .expect("paginated open MRs should succeed");
    assert_eq!(
        mrs.iter().map(|mr| mr.iid).collect::<Vec<_>>(),
        vec![1, 101],
        "page 2's 101st MR must be included"
    );

    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2, "open-MR list should fetch both pages");
    assert!(
        has_query(&requests[0], "state", "opened"),
        "open-MR list must request state=opened: {}",
        requests[0]
    );
    assert!(
        has_query(&requests[0], "per_page", "100"),
        "open-MR list must request per_page=100: {}",
        requests[0]
    );
    assert!(
        has_query(&requests[0], "page", "1"),
        "first open-MR page must request page=1: {}",
        requests[0]
    );
    assert!(
        !has_query(&requests[0], "source_branch", "feature"),
        "cache-filling list must not filter by source_branch: {}",
        requests[0]
    );
    assert!(
        has_query(&requests[1], "state", "opened"),
        "second open-MR page must keep state=opened: {}",
        requests[1]
    );
    assert!(
        has_query(&requests[1], "per_page", "100"),
        "second open-MR page must keep per_page=100: {}",
        requests[1]
    );
    assert!(
        has_query(&requests[1], "page", "2"),
        "second open-MR page must follow x-next-page as page=2: {}",
        requests[1]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn encodes_nested_group_project_path() {
    let (client, server) = mock_client(vec![MockResponse {
        status: reqwest::StatusCode::OK,
        body: "[]".into(),
        next_page: None,
    }]);

    client
        .list_open_mrs(GitLabProjectId::new("group/sub", "repo"))
        .await
        .expect("nested group path should list");

    let requests = server.join().unwrap();
    let path = request_path(&requests[0]);
    assert!(
        path.starts_with("/api/v4/projects/group%2Fsub%2Frepo/merge_requests"),
        "nested group must be a single encoded path: {path}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn empty_page_stops_without_another_request() {
    let (client, server) = mock_client(vec![MockResponse {
        status: reqwest::StatusCode::OK,
        body: "[]".into(),
        next_page: Some("2".into()),
    }]);

    let mrs = client
        .list_open_mrs(GitLabProjectId::new("group", "repo"))
        .await
        .expect("empty page is a complete list");
    assert!(mrs.is_empty(), "empty first page is a complete list");
    assert_eq!(
        server.join().unwrap().len(),
        1,
        "empty page must not fetch x-next-page"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn http_401_and_403_are_http_status_errors() {
    for status in [
        reqwest::StatusCode::UNAUTHORIZED,
        reqwest::StatusCode::FORBIDDEN,
    ] {
        let (client, server) = mock_client(vec![MockResponse {
            status,
            body: r#"{"message":"denied"}"#.into(),
            next_page: None,
        }]);

        let err = client
            .list_open_mrs(GitLabProjectId::new("group", "repo"))
            .await
            .expect_err("non-success must not become an empty list");
        let http = err
            .downcast_ref::<HttpStatusError>()
            .unwrap_or_else(|| panic!("{status} should downcast to HttpStatusError: {err:#}"));
        assert_eq!(
            http.status, status,
            "HttpStatusError must preserve the response status"
        );
        server.join().unwrap();
    }
}

#[tokio::test(flavor = "current_thread")]
async fn exceeding_page_cap_is_an_error() {
    let responses = (1..=MAX_OPEN_MR_PAGES)
        .map(|page| MockResponse {
            status: reqwest::StatusCode::OK,
            body: format!("[{}]", mr_json(page as i64, "feature")),
            next_page: Some((page + 1).to_string()),
        })
        .collect();
    let (client, server) = mock_client(responses);

    let err = client
        .list_open_mrs(GitLabProjectId::new("group", "repo"))
        .await
        .expect_err("cap must not return a successful prefix");
    assert!(
        err.to_string().contains("unsafe pagination"),
        "cap must fail as unsafe pagination, not an incidental earlier error: {err:#}"
    );
    assert_eq!(
        server.join().unwrap().len(),
        MAX_OPEN_MR_PAGES,
        "cap must consume exactly MAX_OPEN_MR_PAGES requests"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn repeated_next_page_is_an_error() {
    let (client, server) = mock_client(vec![MockResponse {
        status: reqwest::StatusCode::OK,
        body: format!("[{}]", mr_json(1, "feature")),
        next_page: Some("1".into()),
    }]);

    let err = client
        .list_open_mrs(GitLabProjectId::new("group", "repo"))
        .await
        .expect_err("repeated x-next-page must not loop");
    assert!(
        err.to_string().contains("unsafe pagination"),
        "repeated page must fail as unsafe pagination: {err:#}"
    );
    assert_eq!(
        server.join().unwrap().len(),
        1,
        "repeated x-next-page must stop after the first page, well below the cap"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn later_page_http_failure_is_an_error() {
    let (client, server) = mock_client(vec![
        MockResponse {
            status: reqwest::StatusCode::OK,
            body: format!("[{}]", mr_json(1, "feature")),
            next_page: Some("2".into()),
        },
        MockResponse {
            status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            body: r#"{"message":"boom"}"#.into(),
            next_page: None,
        },
    ]);

    let err = client
        .list_open_mrs(GitLabProjectId::new("group", "repo"))
        .await
        .expect_err("page 2 5xx must not return page 1 as success");
    assert!(
        err.downcast_ref::<HttpStatusError>().is_some(),
        "later-page HTTP failure should be HttpStatusError: {err:#}"
    );
    server.join().unwrap();
}
