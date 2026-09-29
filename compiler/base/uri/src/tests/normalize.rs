use crate::Uri;

#[test]
fn equivalent_references_normalize_alike() {
    let pairs = [
        ("HTTP://www.Example.COM/", "http://www.example.com/"),
        ("http://example.com:80/", "http://example.com/"),
        ("http://example.com:/", "http://example.com/"),
        ("http://example.com", "http://example.com/"),
        ("https://example.com:443", "https://example.com/"),
        ("http://example.com/%7Euser/%2f%3a", "http://example.com/~user/%2F%3A"),
        ("http://example.com/a/./b/../c", "http://example.com/a/c"),
        ("http://[0:0:0:0:0:0:0:1]:8080/", "http://[::1]:8080/"),
        ("http://%41b.example/", "http://ab.example/"),
        ("http://x/?%61=%3d#%7e", "http://x/?a=%3D#~"),
        ("mailto:A@B", "mailto:A@B"),
    ];
    for (written, normal) in pairs {
        let uri: Uri = written.parse().unwrap();
        let normalized = uri.normalized();
        assert_eq!(normalized.as_str(), normal, "{written}");
        normalized.verify().unwrap();
        assert_eq!(normalized.normalized(), normalized, "{written}: normalizing twice changes nothing");
    }
    let a: Uri = "http://a/%7e".parse().unwrap();
    assert!(a.equivalent(&"HTTP://A:80/~".parse().unwrap()));
}
