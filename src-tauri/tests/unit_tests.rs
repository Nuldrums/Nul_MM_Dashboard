//! Unit tests for pure functions — no database, no HTTP, no async runtime needed.

// ═══════════════════════════════════════════════════════════════════
// 1. URL Parsers (resolve_post_id)
// ═══════════════════════════════════════════════════════════════════

mod url_parsers {
    use app_lib::server::connectors::PlatformConnector;

    // ── Reddit ───────────────────────────────────────────────

    mod reddit {
        use super::*;
        use app_lib::server::connectors::reddit::RedditConnector;

        fn connector() -> RedditConnector {
            RedditConnector::new(
                reqwest::Client::new(),
                String::new(), String::new(), String::new(), String::new(),
            )
        }

        #[test]
        fn standard_post_url() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://www.reddit.com/r/rust/comments/abc123/my_post_title/"),
                Some("abc123".into())
            );
        }

        #[test]
        fn post_url_no_trailing_slash() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://www.reddit.com/r/rust/comments/abc123"),
                Some("abc123".into())
            );
        }

        #[test]
        fn old_reddit_url() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://old.reddit.com/r/SaaS/comments/xyz789/check_this_out/"),
                Some("xyz789".into())
            );
        }

        #[test]
        fn comment_permalink() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://www.reddit.com/r/test/comments/def456/title/comment123"),
                Some("def456".into())
            );
        }

        #[test]
        fn with_query_params() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://www.reddit.com/r/test/comments/aaa111/title/?utm_source=share"),
                Some("aaa111".into())
            );
        }

        #[test]
        fn no_comments_segment() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://www.reddit.com/r/rust/"),
                None
            );
        }

        #[test]
        fn empty_string() {
            let c = connector();
            assert_eq!(c.resolve_post_id(""), None);
        }

        #[test]
        fn unrelated_url() {
            let c = connector();
            assert_eq!(c.resolve_post_id("https://google.com"), None);
        }

        #[test]
        fn alphanumeric_id() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://reddit.com/r/x/comments/A1b2C3/"),
                Some("A1b2C3".into())
            );
        }
    }

    // ── HackerNews ───────────────────────────────────────────

    mod hackernews {
        use super::*;
        use app_lib::server::connectors::hackernews::HackerNewsConnector;

        fn connector() -> HackerNewsConnector {
            HackerNewsConnector::new(reqwest::Client::new())
        }

        #[test]
        fn standard_item_url() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://news.ycombinator.com/item?id=12345678"),
                Some("12345678".into())
            );
        }

        #[test]
        fn with_additional_params() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://news.ycombinator.com/item?id=99999&p=2"),
                Some("99999".into())
            );
        }

        #[test]
        fn non_numeric_id() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://news.ycombinator.com/item?id=abcdef"),
                None
            );
        }

        #[test]
        fn no_id_param() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://news.ycombinator.com/newest"),
                None
            );
        }

        #[test]
        fn empty_id() {
            let c = connector();
            // id= followed by & means empty string, which is not all-digit
            assert_eq!(
                c.resolve_post_id("https://news.ycombinator.com/item?id=&foo=bar"),
                None
            );
        }

        #[test]
        fn id_at_end_of_url() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://news.ycombinator.com/item?id=42"),
                Some("42".into())
            );
        }

        #[test]
        fn mixed_alphanumeric() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://news.ycombinator.com/item?id=123abc"),
                None
            );
        }
    }

    // ── YouTube ──────────────────────────────────────────────

    mod youtube {
        use super::*;
        use app_lib::server::connectors::youtube::YouTubeConnector;

        fn connector() -> YouTubeConnector {
            YouTubeConnector::new(reqwest::Client::new(), String::new())
        }

        #[test]
        fn standard_watch_url() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
                Some("dQw4w9WgXcQ".into())
            );
        }

        #[test]
        fn watch_with_extra_params() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://www.youtube.com/watch?v=abc123&t=42s&list=PLxyz"),
                Some("abc123".into())
            );
        }

        #[test]
        fn shorts_url() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://www.youtube.com/shorts/XyZ789AbC"),
                Some("XyZ789AbC".into())
            );
        }

        #[test]
        fn shorts_with_query() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://www.youtube.com/shorts/XyZ789AbC?feature=share"),
                Some("XyZ789AbC".into())
            );
        }

        #[test]
        fn short_share_url() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://youtu.be/dQw4w9WgXcQ"),
                Some("dQw4w9WgXcQ".into())
            );
        }

        #[test]
        fn short_share_with_timestamp() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://youtu.be/dQw4w9WgXcQ?t=120"),
                Some("dQw4w9WgXcQ".into())
            );
        }

        #[test]
        fn channel_url_no_video() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://www.youtube.com/c/SomeChannel"),
                None
            );
        }

        #[test]
        fn empty_string() {
            let c = connector();
            assert_eq!(c.resolve_post_id(""), None);
        }

        #[test]
        fn embed_url_no_match() {
            // embed URLs use /embed/ID but the parser doesn't handle them (known limitation)
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://www.youtube.com/embed/dQw4w9WgXcQ"),
                None
            );
        }
    }

    // ── Twitter ──────────────────────────────────────────────

    mod twitter {
        use super::*;
        use app_lib::server::connectors::twitter::TwitterConnector;

        fn connector() -> TwitterConnector {
            TwitterConnector::new(reqwest::Client::new(), String::new())
        }

        #[test]
        fn standard_tweet_url() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://twitter.com/user/status/1234567890123456789"),
                Some("1234567890123456789".into())
            );
        }

        #[test]
        fn x_domain() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://x.com/user/status/9876543210"),
                Some("9876543210".into())
            );
        }

        #[test]
        fn with_query_params() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://twitter.com/user/status/111222333?s=20"),
                Some("111222333".into())
            );
        }

        #[test]
        fn with_trailing_path() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://x.com/user/status/111222333/analytics"),
                Some("111222333".into())
            );
        }

        #[test]
        fn non_numeric_id() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://twitter.com/user/status/not_a_number"),
                None
            );
        }

        #[test]
        fn profile_url_no_status() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://twitter.com/someuser"),
                None
            );
        }

        #[test]
        fn empty_string() {
            let c = connector();
            assert_eq!(c.resolve_post_id(""), None);
        }

        #[test]
        fn mobile_url() {
            let c = connector();
            assert_eq!(
                c.resolve_post_id("https://mobile.twitter.com/user/status/555666777"),
                Some("555666777".into())
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// 2. AI Response Parsing (parse_json_response)
// ═══════════════════════════════════════════════════════════════════

mod ai_response_parsing {
    use app_lib::server::ai::analyzer::parse_json_response;

    #[test]
    fn plain_json() {
        let input = r#"{"summary": "All good", "score": 85}"#;
        let result = parse_json_response(input).unwrap();
        assert_eq!(result["summary"], "All good");
        assert_eq!(result["score"], 85);
    }

    #[test]
    fn json_in_code_fence() {
        let input = "```json\n{\"summary\": \"Fenced\", \"score\": 90}\n```";
        let result = parse_json_response(input).unwrap();
        assert_eq!(result["summary"], "Fenced");
        assert_eq!(result["score"], 90);
    }

    #[test]
    fn json_in_plain_code_fence() {
        let input = "```\n{\"key\": \"value\"}\n```";
        let result = parse_json_response(input).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn json_with_leading_trailing_whitespace() {
        let input = "  \n  {\"a\": 1}  \n  ";
        let result = parse_json_response(input).unwrap();
        assert_eq!(result["a"], 1);
    }

    #[test]
    fn fenced_with_whitespace() {
        let input = "  ```json\n  {\"b\": 2}  \n```  ";
        let result = parse_json_response(input).unwrap();
        assert_eq!(result["b"], 2);
    }

    #[test]
    fn complex_nested_json() {
        let input = r#"```json
{
    "summary": "Campaign performing well",
    "top_performers": [{"post_id": "1", "score": 95}],
    "patterns": [{"pattern": "Video outperforms text"}],
    "recommendations": ["Post more video content"]
}
```"#;
        let result = parse_json_response(input).unwrap();
        assert_eq!(result["summary"], "Campaign performing well");
        assert!(result["top_performers"].is_array());
        assert_eq!(result["top_performers"][0]["score"], 95);
    }

    #[test]
    fn malformed_json_returns_error() {
        let input = "this is not json at all";
        assert!(parse_json_response(input).is_err());
    }

    #[test]
    fn empty_string_returns_error() {
        assert!(parse_json_response("").is_err());
    }

    #[test]
    fn fenced_but_malformed_content() {
        let input = "```json\nnot valid json\n```";
        assert!(parse_json_response(input).is_err());
    }

    #[test]
    fn json_array_response() {
        let input = r#"[{"id": 1}, {"id": 2}]"#;
        let result = parse_json_response(input).unwrap();
        assert!(result.is_array());
        assert_eq!(result[0]["id"], 1);
    }

    #[test]
    fn fenced_json_with_backticks_in_content() {
        // Triple backtick fence with JSON that contains a string with backticks
        let input = "```json\n{\"code\": \"use `foo`\"}\n```";
        let result = parse_json_response(input).unwrap();
        assert_eq!(result["code"], "use `foo`");
    }
}

// ═══════════════════════════════════════════════════════════════════
// 3. Tag Serialization / Deserialization
// ═══════════════════════════════════════════════════════════════════

mod tag_serde {
    use app_lib::server::db::models::{
        deserialize_tags_from_input,
        serialize_tags_to_json_string,
        parse_json_column,
    };
    use serde::Deserialize;
    use serde_json::json;

    // Helper struct to test the deserializer via serde
    #[derive(Deserialize, Debug)]
    struct TagsWrapper {
        #[serde(default, deserialize_with = "deserialize_tags_from_input")]
        tags: Option<String>,
    }

    fn deser(val: serde_json::Value) -> Option<String> {
        let wrapper: TagsWrapper = serde_json::from_value(json!({ "tags": val })).unwrap();
        wrapper.tags
    }

    fn deser_missing() -> Option<String> {
        let wrapper: TagsWrapper = serde_json::from_value(json!({})).unwrap();
        wrapper.tags
    }

    // ── deserialize_tags_from_input ──────────────────────────

    #[test]
    fn deser_null_returns_none() {
        assert_eq!(deser(serde_json::Value::Null), None);
    }

    #[test]
    fn deser_missing_field_returns_none() {
        assert_eq!(deser_missing(), None);
    }

    #[test]
    fn deser_array_of_strings() {
        let result = deser(json!(["rust", "marketing", "saas"]));
        assert!(result.is_some());
        let s = result.unwrap();
        let parsed: Vec<String> = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, vec!["rust", "marketing", "saas"]);
    }

    #[test]
    fn deser_empty_array() {
        let result = deser(json!([]));
        assert_eq!(result, Some("[]".into()));
    }

    #[test]
    fn deser_json_string_array() {
        // A string that is itself a JSON array — should be kept as-is
        let result = deser(json!(r#"["a","b"]"#));
        assert!(result.is_some());
        let parsed: Vec<String> = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed, vec!["a", "b"]);
    }

    #[test]
    fn deser_plain_string() {
        // A plain string (not a JSON array) — kept as-is
        let result = deser(json!("marketing"));
        assert_eq!(result, Some("marketing".into()));
    }

    #[test]
    fn deser_mixed_type_array() {
        let result = deser(json!(["text", 42, true]));
        assert!(result.is_some());
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed.len(), 3);
    }

    // ── serialize_tags_to_json_string ────────────────────────

    // Helper to round-trip serialize
    fn ser(tags: Option<String>) -> serde_json::Value {
        #[derive(serde::Serialize)]
        struct W {
            #[serde(serialize_with = "serialize_tags_to_json_string")]
            tags: Option<String>,
        }
        serde_json::to_value(W { tags }).unwrap()
    }

    #[test]
    fn ser_none_is_null() {
        let v = ser(None);
        assert!(v["tags"].is_null());
    }

    #[test]
    fn ser_json_array_string_becomes_array() {
        let v = ser(Some(r#"["a","b","c"]"#.into()));
        assert!(v["tags"].is_array());
        assert_eq!(v["tags"][0], "a");
        assert_eq!(v["tags"][2], "c");
    }

    #[test]
    fn ser_non_json_string_stays_string() {
        let v = ser(Some("not json".into()));
        assert_eq!(v["tags"], "not json");
    }

    #[test]
    fn ser_empty_array_string() {
        let v = ser(Some("[]".into()));
        assert!(v["tags"].is_array());
        assert_eq!(v["tags"].as_array().unwrap().len(), 0);
    }

    // ── parse_json_column ────────────────────────────────────

    #[test]
    fn parse_none_is_null() {
        assert!(parse_json_column(&None).is_null());
    }

    #[test]
    fn parse_valid_json_object() {
        let result = parse_json_column(&Some(r#"{"key": "val"}"#.into()));
        assert_eq!(result["key"], "val");
    }

    #[test]
    fn parse_valid_json_array() {
        let result = parse_json_column(&Some(r#"["a","b"]"#.into()));
        assert!(result.is_array());
        assert_eq!(result[0], "a");
    }

    #[test]
    fn parse_invalid_json_returns_null() {
        let result = parse_json_column(&Some("not json {".into()));
        assert!(result.is_null());
    }

    #[test]
    fn parse_empty_string_returns_null() {
        let result = parse_json_column(&Some("".into()));
        assert!(result.is_null());
    }
}

// ═══════════════════════════════════════════════════════════════════
// 4. FTS5 Query Builder
// ═══════════════════════════════════════════════════════════════════

mod fts_query_builder {
    use app_lib::server::ai::embedder::build_fts_query;

    #[test]
    fn single_word() {
        assert_eq!(build_fts_query("marketing"), "marketing");
    }

    #[test]
    fn multiple_words_become_or() {
        assert_eq!(build_fts_query("marketing strategy"), "marketing OR strategy");
    }

    #[test]
    fn three_words() {
        assert_eq!(
            build_fts_query("best reddit posts"),
            "best OR reddit OR posts"
        );
    }

    #[test]
    fn extra_whitespace_collapsed() {
        assert_eq!(
            build_fts_query("  hello   world  "),
            "hello OR world"
        );
    }

    #[test]
    fn double_quotes_escaped() {
        assert_eq!(
            build_fts_query(r#"search "exact phrase""#),
            r#"search OR ""exact OR phrase"""#
        );
    }

    #[test]
    fn tabs_and_newlines_as_whitespace() {
        assert_eq!(
            build_fts_query("hello\tworld\nnew"),
            "hello OR world OR new"
        );
    }

    #[test]
    fn single_character() {
        assert_eq!(build_fts_query("a"), "a");
    }

    #[test]
    fn numeric_query() {
        assert_eq!(build_fts_query("2026 metrics"), "2026 OR metrics");
    }
}
