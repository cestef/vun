use crate::{Secure, Unfetched};

fn follow(from: &str, location: &str) -> Result<String, Unfetched> {
    from.parse::<Secure>()?.follow(location.as_bytes()).map(|s| s.to_string())
}

#[test]
fn only_https_is_requested() {
    assert!("https://forge.example/a.tar.zst".parse::<Secure>().is_ok());
    assert!(matches!("http://forge.example/a.tar.zst".parse::<Secure>(), Err(Unfetched::Insecure(_))));
    assert!(matches!("/a.tar.zst".parse::<Secure>(), Err(Unfetched::Unparsed(_))));
    assert!(matches!("https:///a.tar.zst".parse::<Secure>(), Err(Unfetched::Insecure(_))));
}

#[test]
fn a_redirect_is_read_against_the_url_it_leaves() {
    let from = "https://forge.example/pkg/1.0/a.tar.zst?download=1#top";
    assert_eq!(follow(from, "https://cdn.example/x").unwrap(), "https://cdn.example/x");
    assert_eq!(follow(from, "//cdn.example/x?y").unwrap(), "https://cdn.example/x?y");
    assert_eq!(follow(from, "/x").unwrap(), "https://forge.example/x");
    assert_eq!(follow(from, "b.tar.zst").unwrap(), "https://forge.example/pkg/1.0/b.tar.zst");
    assert_eq!(follow(from, "?raw=1").unwrap(), "https://forge.example/pkg/1.0/a.tar.zst?raw=1");
    assert_eq!(follow(from, "../2.0/./a.tar.zst#x").unwrap(), "https://forge.example/pkg/2.0/a.tar.zst");
}

#[test]
fn a_redirect_leaving_https_is_refused_before_it_is_requested() {
    let from = "https://forge.example/a.tar.zst";
    assert!(matches!(follow(from, "http://forge.example/a.tar.zst"), Err(Unfetched::Insecure(_))));
    assert!(matches!(follow(from, "HTTP://forge.example/a"), Err(Unfetched::Insecure(_))));
    assert!(matches!(follow(from, "ftp://forge.example/a"), Err(Unfetched::Insecure(_))));
}
