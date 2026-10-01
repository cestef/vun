use super::*;

fn env() -> Env {
    Env::default()
}

#[test]
fn auto_follows_the_terminal_unless_the_environment_decides() {
    let auto = ColorChoice::Auto;
    assert_eq!(auto.color(env(), true), Color::Always);
    assert_eq!(auto.color(env(), false), Color::Never);
    assert_eq!(auto.color(Env(Some(Color::Never)), true), Color::Never);
    assert_eq!(auto.color(Env(Some(Color::Always)), false), Color::Always);
    assert_eq!(ColorChoice::Always.color(Env(Some(Color::Never)), false), Color::Always);
    assert_eq!(ColorChoice::Never.color(env(), true), Color::Never);
    assert_eq!(ColorChoice::named(Some("never")), ColorChoice::Never);
    assert_eq!(ColorChoice::named(None), ColorChoice::Auto);
}

#[test]
fn text_is_plain_through_display() {
    let text = Text::new().styled(Style::Literal, "vun").plain(" ").styled(Style::Placeholder, "<file>");
    assert_eq!(text.to_string(), "vun <file>");
    assert_eq!(text.width(), 10);
    assert_eq!(Text::join([Text::from("a"), Text::from("b")], ", ").to_string(), "a, b");
}

fn printed(color: ColorChoice, text: &Text) -> Vec<u8> {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    {
        let mut shell = Shell::new(Out::new(&mut out, false), Out::new(&mut err, false), env());
        shell.choose(color);
        shell.out.print(text).unwrap();
    }
    out
}

#[test]
fn a_stream_styles_only_with_colors() {
    let text = Text::new().styled(Style::Error, "no");
    assert_eq!(printed(ColorChoice::Never, &text), b"no");
    let ansi = Style::Error.ansi();
    let styled = format!("{}no{}", ansi.render(), ansi.render_reset());
    assert_eq!(printed(ColorChoice::Always, &text), styled.as_bytes());
}

const COLUMNS: [Column; 2] = [Column::left("name"), Column::right("size")];
const BARE: [Column; 2] = [Column::left(""), Column::left("")];
const ERROR: Noun = Noun { one: "error", many: "errors" };

#[test]
fn tables_align_columns_and_show_heads() {
    let mut table = Table::new(&COLUMNS).indent(1);
    table.row([Text::from("a"), Text::from("1234")]);
    table.row([Text::from("long"), Text::from("5")]);
    assert_eq!(table.text().to_string(), " name  size\n a     1234\n long     5\n");
    let mut table = Table::new(&BARE);
    table.row([Text::from("x"), Text::from("last")]);
    assert_eq!(table.text().to_string(), "x  last\n");
}

/// A status as tests report it.
struct Done(u64);

impl Record for Done {
    fn kind(&self) -> &'static str {
        "done"
    }

    fn text(&self) -> Text {
        Text::from(format!("in {}s", self.0))
    }

    fn fields(&self) -> vun_data::Object<'static> {
        vun_data::Object::new().with("seconds", self.0)
    }
}

impl Status for Done {
    fn verb(&self) -> Verb {
        Verb::good("Finished")
    }
}

/// What a shell in `format` writes on each stream for a status, then for it quiet.
fn reported(format: Format) -> (String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    {
        let mut shell = Shell::plain(&mut out, &mut err);
        shell.show(format);
        shell.status(&Done(1));
        shell.verbosity = Verbosity::Quiet;
        shell.status(&Done(2));
    }
    (String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

#[test]
fn statuses_are_lines_for_people_and_json_for_programs() {
    assert_eq!(reported(Format::Human), (String::new(), "    Finished in 1s\n".to_owned()));
    let (out, err) = reported(Format::Json);
    assert_eq!((vun_data::json::parse(out.trim()).unwrap(), err), (Done(1).json(), String::new()));
}

struct Broke;

impl Record for Broke {
    fn kind(&self) -> &'static str {
        "broke"
    }
    fn text(&self) -> Text {
        Text::from("it")
    }
    fn fields(&self) -> vun_data::Object<'static> {
        vun_data::Object::new()
    }
}

impl Status for Broke {
    fn verb(&self) -> Verb {
        Verb::failed("Failed")
    }
}

#[test]
fn quiet_hides_progress_but_not_failures() {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    {
        let mut shell = Shell::plain(&mut out, &mut err);
        shell.verbosity = Verbosity::Quiet;
        shell.status(&Done(1));
        shell.status(&Broke);
    }
    assert_eq!(String::from_utf8(err).unwrap(), "      Failed it\n");
}

#[test]
fn units_read_as_cargo_shows_them() {
    use std::time::Duration;
    assert_eq!(Elapsed(Duration::from_millis(420)).to_string(), "0.42s");
    assert_eq!(Elapsed(Duration::from_secs(63)).to_string(), "1m 03s");
    assert_eq!(Bytes(512).to_string(), "512 B");
    assert_eq!(Bytes(3 * 1024 * 1024 + 400 * 1024).to_string(), "3.4 MiB");
    assert_eq!(
        (ERROR.count(1).to_string(), ERROR.count(2).to_string()),
        ("1 error".into(), "2 errors".into())
    );
}

#[test]
fn markdown_keeps_its_text_and_styles_code() {
    let page = "# id\n\nUse `f`.\n\n```console\n$ vun check a.vun\nerror\n```\n";
    let text = Text::markdown(page);
    assert_eq!(text.to_string(), page);
    let styled = |style| {
        text.pieces().iter().filter(|p| p.style == Some(style)).map(|p| p.text.as_ref()).collect::<Vec<_>>()
    };
    assert_eq!(styled(Style::Heading), ["# id"]);
    assert_eq!(styled(Style::Literal), ["f", "$ vun check a.vun"]);
}

#[test]
fn styled_lines_are_pieces_on_the_stream_people_read() {
    use vun_data::{Object, ToData};
    let piece = |style, text: &'static str| Piece { style, text: text.into() };
    let line = |stream: Stream, pieces: Vec<Piece>| {
        let object = Object::new()
            .with("type", "line")
            .with("stream", stream.to_data())
            .with("pieces", pieces.to_data());
        vun_data::Value::Object(object)
    };
    let (out, err) = reported(Format::Styled);
    let finished = vec![
        piece(None, "    "),
        piece(Some(Style::Good), "Finished"),
        piece(None, " "),
        piece(None, "in 1s"),
    ];
    assert_eq!(
        (out, vun_data::json::parse(err.trim()).unwrap()),
        (String::new(), line(Stream::Stderr, finished))
    );

    let (mut out, mut bytes) = (Vec::new(), Vec::new());
    {
        let mut shell = Shell::plain(&mut out, &mut bytes);
        shell.show(Format::Styled);
        write!(shell.err.writer(), "plain\nunended").unwrap();
    }
    let written = String::from_utf8(bytes).unwrap();
    let lines: Vec<_> = written.lines().map(|l| vun_data::json::parse(l).unwrap()).collect();
    let plain = |text| line(Stream::Stderr, vec![piece(None, text)]);
    assert_eq!(lines, [plain("plain"), plain("unended")]);
}
