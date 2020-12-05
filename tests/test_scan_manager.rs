mod utils;
use assert_cmd::Command;
use httpmock::Method::GET;
use httpmock::{Mock, MockServer};
use predicates::prelude::*;
use utils::{setup_tmp_directory, teardown_tmp_directory};

#[test]
/// pass a known serialized scan with 1 scan complete and 1 not. expect the incomplete scan to
/// start and the complete to not start. expect the responses, scans, and configuration structures
/// to be populated based off the contents of the given state file
fn resume_scan_works() {
    let srv = MockServer::start();
    let (tmp_dir, file) =
        setup_tmp_directory(&["css".to_string(), "stuff".to_string()], "wordlist").unwrap();

    // localhost:PORT/ <- complete
    // localhost:PORT/js <- will get scanned with /css and /stuff
    let complete_scan = format!(
        r#"{{"id":"057016a14769414aac9a7a62707598cb","url":"{}","scan_type":"Directory","complete":true}}"#,
        srv.url("/")
    );
    let incomplete_scan = format!(
        r#"{{"id":"400b2323a16f43468a04ffcbbeba34c6","url":"{}","scan_type":"Directory","complete":false}}"#,
        srv.url("/js")
    );
    let scans = format!(r#""scans":[{},{}]"#, complete_scan, incomplete_scan);

    let config = format!(
        r#""config": {{"type":"configuration","wordlist":"{}","config":"","proxy":"","replay_proxy":"","target_url":"{}","status_codes":[200,204,301,302,307,308,401,403,405],"replay_codes":[200,204,301,302,307,308,401,403,405],"filter_status":[],"threads":50,"timeout":7,"verbosity":0,"quiet":false,"json":false,"output":"","debug_log":"","user_agent":"feroxbuster/1.9.0","redirects":false,"insecure":false,"extensions":[],"headers":{{}},"queries":[],"no_recursion":false,"extract_links":false,"add_slash":false,"stdin":false,"depth":2,"scan_limit":1,"filter_size":[],"filter_line_count":[],"filter_word_count":[],"filter_regex":[],"dont_filter":false}}"#,
        file.to_string_lossy(),
        srv.url("/")
    );

    // // localhost:PORT/js/css has already been seen, expect not to be scanned
    let response = format!(
        r#"{{"type":"response","url":"{}","path":"/js/css","wildcard":true,"status":301,"content_length":173,"line_count":10,"word_count":16,"headers":{{"server":"nginx/1.16.1"}}}}"#,
        srv.url("/js/css")
    );
    let responses = format!(r#""responses":[{}]"#, response);

    // not scanned because /js is not complete, and /js/stuff response is not known
    let not_scanned_yet = Mock::new()
        .expect_method(GET)
        .expect_path("/js/stuff")
        .return_status(200)
        .return_body("i expect to be scanned")
        .create_on(&srv);

    // will get scanned because /js is not complete, but because response of /js/css is known, the
    // response will not be in stdout
    let already_scanned = Mock::new()
        .expect_method(GET)
        .expect_path("/js/css")
        .return_status(200)
        .create_on(&srv);

    // already scanned because scan on / is complete
    let also_already_scanned = Mock::new()
        .expect_method(GET)
        .expect_path("/css")
        .return_status(200)
        .return_body("two words")
        .create_on(&srv);

    let state_file_contents = format!("{{{},{},{}}}", scans, config, responses);
    let (tmp_dir2, state_file) = setup_tmp_directory(&[state_file_contents], "state-file").unwrap();

    Command::cargo_bin("feroxbuster")
        .unwrap()
        .arg("--resume-from")
        .arg(state_file.as_os_str())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("/js/stuff")
                .and(predicate::str::contains("22c"))
                .and(predicate::str::contains("5w"))
                .and(predicate::str::contains("/js/css"))
                .not()
                .and(predicate::str::contains("2w"))
                .not()
                .and(predicate::str::contains("9c"))
                .not(),
        );

    teardown_tmp_directory(tmp_dir);
    teardown_tmp_directory(tmp_dir2);

    assert_eq!(already_scanned.times_called(), 1);
    assert_eq!(also_already_scanned.times_called(), 0);
    assert_eq!(not_scanned_yet.times_called(), 1);
}
